use std::{
    collections::HashMap,
    env::{self, current_dir},
    fs::{self, File, read_to_string},
    path::PathBuf,
    str::FromStr,
    sync::Arc,
};

use color_eyre::eyre::Error as ColorError;
use futures::future::try_join_all;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use jiff::Timestamp;
use log::{debug, info, warn};
use prettytable::{Table, row};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

use crate::{
    EntryError, RegistryError, ValidationError,
    data::{DownloadStatus, RefDataset},
    downloads::request_dataset,
    validate::UnvalidatedFile,
};

/// A reference manager for all data associated with your bioinformatics project.
///
/// Projects are the top-level abstraction in refman, allowing you to register, track,
/// download, and manage reference files like FASTA, Genbank, GFA, GFF, GTF and BED files
/// for your bioinformatics work. A Project maintains a registry of datasets, where each dataset
/// has a unique label and can contain references to multiple file types.
///
/// The Project struct provides methods to:
/// - Initialize new reference management projects
/// - Register new datasets or update existing ones
/// - Download registered datasets from remote URLs
/// - Remove datasets from the registry
/// - Pretty print the current state of registered datasets
///
/// Projects can be either local (stored in ./refman.toml) or global (stored in ~/.refman/refman.toml).
/// The registry location can also be customized via the `REFMAN_HOME` environment variable.
///
/// Each dataset in a project is tracked with a unique label and can contain optional URLs pointing
/// to reference files in standard bioinformatics formats (FASTA, Genbank, GFA, GFF, GTF, BED).
/// The registry maintains metadata like when it was last modified and optional title/description fields.
///
/// # Examples
///
/// ```no_run
/// # use refman::project::Project;
/// // Create a new local project
/// let project = Project::new(
///     Some("My Assembly Project".to_string()),
///     Some("Reference data for genome assembly".to_string()),
///     false
/// );
/// ```
///
/// The Project struct integrates with other refman types like `RefDataset` for managing individual
/// reference datasets and `RegistryOptions` for configuring where and how the registry is stored.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct Project {
    project: Registry,
}

type MultiDownloadResults = Vec<Result<UnvalidatedFile, ColorError>>;

impl Project {
    /// Creates a new Project struct with optional title and description strings and
    /// a boolean flag controlling if the project's registry file is placed in a
    /// global location (`REFMAN_HOME`or ~/.refman) or locally (./refman.toml).
    ///
    /// A Project is the top-level struct for managing reference data in refman. It
    /// maintains a registry of reference genomics datasets, where each dataset can
    /// include references to multiple standard bioinformatics file formats like
    /// FASTA, Genbank, GFA, GFF, GTF and BED files. The registry stores metadata
    /// about each reference dataset including when it was last modified.
    ///
    /// The registry file location depends on the `global` parameter:
    /// - If `global=false` (default), creates a local refman.toml in current directory
    /// - If `global=true`, uses either `$REFMAN_HOME/.refman/refman.toml` or ~/.refman/refman.toml
    ///
    /// # Arguments
    ///
    /// * `title` - Optional title for the project
    /// * `description` - Optional description of the project
    /// * `global` - Whether to store the registry file globally or locally
    ///
    /// # Returns
    ///
    /// Returns a new Project instance initialized with the provided title, description
    /// and global flag. The internal Registry is created with default values for
    /// `last_modified` timestamp and an empty datasets vector.
    fn new(title: Option<String>, description: Option<String>, global: bool) -> Self {
        // fill in any user provided title, description, or global information on
        // top of the information stored in a project by default
        let registry = Registry {
            title,
            description,
            global,
            ..Registry::default()
        };

        Self { project: registry }
    }

    /// Returns a read-only slice of all reference datasets currently registered in the project.
    ///
    /// This method provides access to the raw collection of `RefDataset` entries stored in the
    /// project's registry. Each `RefDataset` represents a labeled collection of bioinformatics
    /// reference files, potentially including FASTA, Genbank, GFA, GFF, GTF and BED formats.
    ///
    /// This accessor is useful for:
    /// - Inspecting the currently registered datasets without modifying them
    /// - Iterating over registered datasets to check their properties
    /// - Filtering datasets based on custom criteria
    /// - Accessing individual dataset labels and file URLs
    ///
    /// The returned slice allows read-only access to ensure the registry's integrity is maintained.
    /// For mutable access, use `datasets_mut()` instead. For taking ownership of the datasets,
    /// use `datasets_owned()`.
    ///
    /// # Returns
    ///
    /// A read-only slice containing all registered `RefDataset` entries in the project.
    /// Returns an empty slice if no datasets are registered.
    #[inline]
    #[must_use]
    pub fn datasets(&self) -> &[RefDataset] {
        self.project.datasets.as_slice()
    }

    /// Returns a mutable slice of all reference datasets registered in the project.
    ///
    /// This method provides mutable access to the raw collection of `RefDataset` entries stored in
    /// the project's registry. Each `RefDataset` represents a labeled collection of bioinformatics
    /// reference files, potentially including FASTA, Genbank, GFA, GFF, GTF and BED formats.
    ///
    /// Mutable access allows modifying existing datasets, for example to:
    /// - Update file URLs for existing datasets
    /// - Modify dataset labels or other metadata
    /// - Add or remove file references from datasets
    /// - Reorder datasets within the registry
    ///
    /// Use this method with caution as it allows direct mutation of the registry state.
    /// For read-only access, use `datasets()` instead. To take ownership of the datasets,
    /// use `datasets_owned()`.
    ///
    /// # Returns
    ///
    /// A mutable slice containing all registered `RefDataset` entries in the project.
    /// Returns an empty slice if no datasets are registered.
    #[inline]
    pub fn datasets_mut(&mut self) -> &mut [RefDataset] {
        self.project.datasets.as_mut_slice()
    }

    /// Takes ownership of all reference datasets registered in the project.
    ///
    /// This method provides a way to take ownership of the raw collection of `RefDataset` entries
    /// stored in the project's registry, consuming the project in the process. Each `RefDataset`
    /// represents a labeled collection of bioinformatics reference files, potentially including
    /// FASTA, Genbank, GFA, GFF, GTF and BED formats.
    ///
    /// Taking ownership via `datasets_owned()` allows:
    /// - Moving datasets out of the Project context entirely
    /// - Transferring datasets between Projects
    /// - Performing owned operations on datasets that require ownership
    /// - Converting datasets into other data structures
    ///
    /// This is different from `datasets()` which provides read-only access an`datasets_mut()`
    /// which provides mutable access but keeps ownership within the Project. Using `datasets_owned()`
    /// consumes the Project instance.
    ///
    /// # Returns
    ///
    /// A Vec containing all registered `RefDataset` entries, transferring ownership from
    /// the Project to the caller. Returns an empty Vec if no datasets were registered.
    /// The Project instance is consumed in the process.
    #[inline]
    #[must_use]
    pub fn datasets_owned(self) -> Vec<RefDataset> {
        self.project.datasets
    }

    /// Returns a reference to a specific dataset from the Project's registry by its label.
    ///
    /// This method provides direct access to individual reference datasets stored in the project's
    /// registry. It takes a label string and returns a reference to the matching `RefDataset` if one
    /// exists. Each dataset in a refman Project has a unique label that identifies it, containing
    /// optional references to various bioinformatics file formats (FASTA, Genbank, GFA, GFF, GTF, BED).
    ///
    /// The method enforces that:
    /// - The label must exactly match a registered dataset (case-sensitive)
    /// - Only one dataset can have a given label (unique key constraint)
    /// - The dataset must exist in the registry
    ///
    /// This is commonly used to:
    /// - Check details of specific registered datasets
    /// - Access dataset file URLs before downloading
    /// - Verify dataset registration status
    /// - Extract dataset metadata
    ///
    /// The method complements other Project methods like `register()` an`download_dataset()`() in the
    /// dataset management lifecycle. While those methods add and fetch datasets, `get_dataset()`
    /// provides read access to verify and inspect registered data.
    ///
    /// # Arguments
    ///
    /// * `label` - The unique label identifying the dataset to retrieve
    ///
    /// # Returns
    ///
    /// Returns Ok(&RefDataset) with a reference to the matching dataset if found.
    /// Returns `EntryError::LabelNotFound` if no dataset matches the provided label.
    ///
    /// # Errors
    ///
    /// Can return `EntryError::LabelNotFound` if the requested dataset label is not
    /// registered in the project.
    ///
    /// # Panics
    ///
    /// This method will panic if:
    /// - More than one dataset with the same label exists in the registry
    ///   (indicates invalid state as labels must be unique)
    /// - The filtered dataset collection contains an unexpected number of matches
    ///   (should be exactly 1 match for a valid label)
    #[inline]
    pub fn get_dataset(&self, label: &str) -> Result<&RefDataset, EntryError> {
        // pull in a read-only slice of the datasets currently in project state
        let datasets = self.datasets();

        // If a dataset isn't in the current project state, return a refman error
        // wrapped in an anyhow error.
        if datasets
            .iter()
            .map(|dataset| dataset.label.as_str())
            .filter(|ds_label| *ds_label == label)
            .collect::<Vec<&str>>()
            .is_empty()
        {
            Err(EntryError::LabelNotFound(label.to_string()))?;
        }

        // make sure only one dataset matches the provided label, which must be a unique
        // key
        let entry: Vec<_> = datasets
            .iter()
            .filter(|dataset| dataset.label == label)
            .collect();
        assert_eq!(entry.len(), 1);

        Ok(entry[0])
    }

    /// Returns a vector of all registered file URLs for a dataset with the given label.
    ///
    /// This method provides access to all file URLs registered for a dataset, combining any valid URLs
    /// across the supported bioinformatics file formats (FASTA, Genbank, GFA, GFF, GTF, BED). The URLs
    /// can then be used to download reference files, validate dataset completeness, or inspect available
    /// file formats.
    ///
    /// The method will:
    /// - Verify the dataset exists by the given label
    /// - Extract all non-None URLs registered for that dataset
    /// - Return them as a vector in a consistent order (FASTA, Genbank, etc.)
    ///
    /// This complements other dataset access methods by providing URL-specific functionality. While
    /// `get_dataset()` returns the full dataset struct an`download_dataset()` handles file fetching,
    /// `get_dataset_urls()` focuses specifically on URL access and validation.
    ///
    /// The method is used internally by `download_dataset()` to determine which files to fetch, but can
    /// also be used directly to:
    /// - Preview what files are available before downloading
    /// - Extract URLs for custom download handling
    /// - Verify dataset completeness
    /// - Share dataset URLs
    ///
    /// # Arguments
    ///
    /// * `label` - The unique label identifying the dataset whose URLs should be retrieved
    ///
    /// # Returns
    ///
    /// Returns Ok(Vec<String>) containing all non-None URLs registered for the dataset.
    /// Returns an empty vector if the dataset exists but has no URLs registered.
    /// Returns `EntryError::LabelNotFound` if no dataset matches the provided label.
    ///
    /// # Errors
    ///
    /// Can return `EntryError::LabelNotFound` if the requested dataset label is not in the registry.
    #[inline]
    pub fn get_dataset_urls(&self, label: &str) -> Result<Vec<String>, EntryError> {
        // access the dataset for the provided label
        let dataset = self.get_dataset(label)?;

        // build a vector based on the URLs that may or may not be available for downloading
        let urls = vec![
            dataset.fasta.clone(),
            dataset.genbank.clone(),
            dataset.gfa.clone(),
            dataset.gff.clone(),
            dataset.gtf.clone(),
            dataset.bed.clone(),
        ]
        .into_iter()
        .flatten()
        .map(|download| download.url_owned())
        .collect::<Vec<String>>();

        Ok(urls)
    }

    /// Returns a vector of URLs for all reference data across all registered datasets.
    ///
    /// This method provides access to all file URLs registered in the project's datasets,
    /// aggregating URLs from each dataset and each supported bioinformatics file format
    /// (FASTA, Genbank, GFA, GFF, GTF, BED). It is useful for:
    /// - Getting an overview of all reference data in the project
    /// - Batch downloading all registered files
    /// - Validating URLs across the entire registry
    /// - Sharing/exporting full URL lists
    ///
    /// The method processes each dataset sequentially, collecting any non-None URLs into
    /// a single vector. URLs are gathered in a consistent order per dataset:
    /// FASTA -> Genbank -> GFA -> GFF -> GTF -> BED.
    ///
    /// Unlike `get_dataset_urls()` which operates on a single labeled dataset, this method
    /// provides complete URL access across the entire registry. It complements other Project
    /// methods like `download_dataset()` by enabling bulk operations across all reference data.
    ///
    /// The method enforces URL validity by checking that:
    /// - No empty URLs are included
    /// - All URLs use either http:// or https:// protocols
    ///
    /// # Returns
    ///
    /// Returns Ok(Vec<String>) containing all valid URLs across all datasets.
    /// Returns an empty vector if no URLs are registered.
    ///
    /// # Errors
    ///
    /// Can return `EntryError` variants if:
    /// - Dataset access fails
    /// - URL validation fails
    /// - Project state is invalid
    ///
    /// # Panics
    ///
    /// This method will panic if:
    /// - Empty URLs are found in datasets (invalid state)
    /// - URLs with invalid protocols are found (must be http/https)
    #[inline]
    pub fn get_all_urls(&self) -> Result<Vec<String>, EntryError> {
        // access the dataset for the provided label
        let datasets = self.datasets();

        // build a vector based on the URLs that may or may not be available for downloading
        let mut all_urls = Vec::new();
        for dataset in datasets {
            let urls = vec![
                dataset.fasta.clone(),
                dataset.genbank.clone(),
                dataset.gfa.clone(),
                dataset.gff.clone(),
                dataset.gtf.clone(),
                dataset.bed.clone(),
            ]
            .into_iter()
            .flatten()
            .map(|download| download.url_owned())
            .collect::<Vec<String>>();
            all_urls.extend(urls);
        }
        assert!(
            all_urls.iter().all(|url| !url.is_empty()),
            "Found empty URLs in dataset"
        );
        assert!(
            all_urls
                .iter()
                .all(|url| url.starts_with("http://") || url.starts_with("https://")),
            "Found invalid URL protocols"
        );

        Ok(all_urls)
    }

    /// Checks if a dataset with a given label is registered in the project.
    ///
    /// This method searches through the project's registry to determine if a dataset
    /// with the specified label exists. Each dataset in a refman Project must have a
    /// unique label that identifies it - this label acts as the primary key for the
    /// dataset within the registry.
    ///
    /// This method is useful for:
    /// - Validating labels before attempting to register or update datasets
    /// - Checking existence of specific datasets before trying to download them
    /// - General queries about what data is available in the project
    ///
    /// The check is case-sensitive - "genome" and "Genome" are considered different labels.
    /// Labels must be unique within a project's registry.
    ///
    /// # Arguments
    ///
    /// * `label` - The label string to search for in the registry
    ///
    /// # Returns
    ///
    /// Returns `true` if a dataset with the given label exists in the registry,
    /// `false` otherwise. Note that this only checks for label existence, not whether
    /// the dataset has any file URLs registered or if those files are accessible.
    #[must_use]
    pub fn is_registered(&self, label: &str) -> bool {
        // Iterate through a slice of the available datasets, keeping only the dataset
        // with a label matching what the user has requested. Return true if the result
        // is not empty and false if it is.
        !self
            .datasets()
            .iter()
            .filter(|dataset| dataset.label == label)
            .collect::<Vec<&RefDataset>>()
            .is_empty()
    }

    /// Registers a new dataset or updates an existing dataset in the Project's registry.
    ///
    /// This is one of the core methods for managing reference data in refman. It takes a `RefDataset`
    /// struct containing a unique label and optional URLs for various bioinformatics file formats
    /// (FASTA, Genbank, GFA, GFF, GTF, BED) and either:
    ///
    /// - Adds it as a new dataset if the label doesn't exist in the registry yet
    /// - Updates an existing dataset with any new URLs provided if the label matches
    ///
    /// When updating an existing dataset, only fields that are Some(url) in the new `RefDataset`
    /// will overwrite the existing dataset's fields. This allows for incremental updates where
    /// you can add new file references to a dataset over time without having to re-specify
    /// existing URLs.
    ///
    /// The registry enforces that dataset labels must be unique - you cannot have two datasets
    /// with the same label. This allows the label to act as a primary key for looking up and
    /// managing datasets within the project.
    ///
    /// # Arguments
    ///
    /// * `new_dataset` - A `RefDataset` struct containing the label and optional file URLs to
    ///   register or update. The label field is required and must be unique within the registry.
    ///
    /// # Returns
    ///
    /// Returns Ok(Project) with the updated Project if registration succeeds, or an `EntryError`
    /// if there are issues with the dataset registration (e.g. invalid state detected).
    ///
    /// # Examples
    ///
    /// To register a new dataset:
    /// ```rust,no_run
    /// # use refman::{project::Project, data::RefDataset};
    /// let mut project = Project::new(None, None, false);
    /// let dataset = RefDataset {
    ///     label: "genome".into(),
    ///     fasta: Some("https://example.com/genome.fasta".into()),
    ///     ..Default::default()
    /// };
    /// project = project.register(dataset).unwrap();
    /// ```
    ///
    /// The registration process will either add this as a new dataset if "genome" is not yet
    /// registered, or update the existing "genome" dataset with the new FASTA URL if it exists.
    ///
    /// # Errors
    ///
    /// This method can return several types of errors:
    /// - `EntryError::LabelNotFound` if the dataset being registered cannot be found during updates
    /// - `EntryError::FinalEntry` if registering this dataset would leave the registry empty
    /// - Filesystem errors from reading/writing the registry file
    /// - Serialization errors when encoding/decoding the registry TOML
    /// - Permission errors when accessing registry files
    /// - IO errors if registry files or directories cannot be accessed
    /// - Environment variable errors if `REFMAN_HOME` is invalid
    /// - Path resolution errors for invalid registry paths
    ///
    /// # Panics
    ///
    /// This method will panic if multiple datasets matching the given label are found in
    /// the registry. This should never happen as labels must be unique, but represents an
    /// invalid state that requires immediate attention.
    ///
    pub fn register(mut self, new_dataset: RefDataset) -> Result<Self, EntryError> {
        // find the index of the old dataset to be updated with new information from
        // the user
        let old_dataset_index: Vec<_> = self
            .datasets()
            .iter()
            .enumerate()
            .filter(|(_i, dataset)| *dataset.label == new_dataset.label)
            .map(|(i, _)| i)
            .collect();

        // if the label wasn't found, it's not in the registry, so it can be safely
        // appended without any fear of duplication
        if old_dataset_index.is_empty() {
            self.project.datasets.push(new_dataset);
            return Ok(self);
        }

        // Make sure that the above system that we *assume* will work doesn't actually break (it should never
        // be possible to have two dataset entries with the same label).
        assert_eq!(
            old_dataset_index.len(),
            1,
            "Invalid state slipped through the cracks when identifying which dataset should be updated with the new information for dataset '{}'. Somehow, multiple indices were returned: {:?}",
            &new_dataset.label,
            &old_dataset_index
        );

        // With that assert passing, pull out the index usize
        let old_dataset_index = old_dataset_index[0];

        // pull in a mutable reference to the slice of datasets, get a mutable reference to the one
        // dataset we need to update (using the index), and then update each of it's fields if the
        // user provided values for them.
        let datasets = self.datasets_mut();
        let dataset_to_update = &mut datasets[old_dataset_index];
        if new_dataset.fasta.is_some() {
            dataset_to_update.fasta = new_dataset.fasta;
        }
        if new_dataset.genbank.is_some() {
            dataset_to_update.genbank = new_dataset.genbank;
        }
        if new_dataset.gfa.is_some() {
            dataset_to_update.gfa = new_dataset.gfa;
        }
        if new_dataset.gff.is_some() {
            dataset_to_update.gff = new_dataset.gff;
        }
        if new_dataset.gtf.is_some() {
            dataset_to_update.gtf = new_dataset.gtf;
        }
        if new_dataset.bed.is_some() {
            dataset_to_update.bed = new_dataset.bed;
        }

        // If we've made it this far, all is well; return the mutated instance of
        // the project.
        Ok(self)
    }

    #[allow(clippy::similar_names)]
    pub(crate) fn get_downloads_per_dataset(
        &self,
        label: Option<&str>,
    ) -> Vec<(RefDataset, Vec<UnvalidatedFile>)> {
        let datasets = if let Some(label) = label {
            self.clone()
                .datasets_owned()
                .into_iter()
                .filter(|dataset| dataset.label == label)
                .collect::<Vec<_>>()
        } else {
            self.clone()
                .datasets_owned()
                .into_iter()
                .collect::<Vec<_>>()
        };
        datasets
            .into_iter()
            .map(|dataset| {
                let fasta = dataset.get_fasta_download();
                let genbank = dataset.get_genbank_download();
                let gfa = dataset.get_gfa_download();
                let gtf = dataset.get_gtf_download();
                let gff = dataset.get_gff_download();
                let bed = dataset.get_bed_download();
                let files = vec![fasta, genbank, gfa, gff, gtf, bed]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
                (dataset, files)
            })
            .collect::<Vec<_>>()
    }

    /// Downloads a reference dataset from a Project's registry by label, fetching any registered file
    /// URLs into a target directory.
    ///
    /// This method implements the core file downloading functionality in refman. Given a dataset label
    /// and target directory, it will:
    /// 1. Verify the dataset exists in the registry
    /// 2. Extract all registered file URLs for that dataset (FASTA, Genbank, GFA, GFF, GTF, BED)
    /// 3. Launch concurrent downloads of all files into the target directory
    /// 4. Handle any download failures or errors
    ///
    /// Downloads happen asynchronously and in parallel for improved performance. The method uses
    /// tokio for async runtime and reqwest for HTTP requests. Files are downloaded maintaining
    /// their original filenames from the URLs.
    ///
    /// Dataset labels must exactly match what is registered (case-sensitive). The target directory
    /// will be created if it doesn't exist. Existing files in the target directory may be
    /// overwritten.
    ///
    /// This is used to fetch reference data after registering datasets with `register()`.
    /// For example, after registering genome data with FASTA and GFF URLs, this method would
    /// concurrently download both files locally.
    ///
    /// # Arguments
    ///
    /// * `label` - The unique label of the dataset to download, must match what was registered
    /// * `target_dir` - Directory path where downloaded files should be saved
    ///
    /// # Returns
    ///
    /// Returns Ok(()) if all downloads complete successfully, or an error if:
    /// - The dataset label is not found in the registry
    /// - Any file downloads fail
    /// - The target directory cannot be accessed/created
    /// - Other IO or HTTP errors occur
    ///
    /// # Errors
    ///
    /// This method can return `EntryError::LabelNotFound` if the dataset is not in the registry,
    /// as well as various IO and HTTP errors wrapped in `anyhow::Error` for failed downloads.
    ///
    /// # Panics
    ///
    /// This method will panic if:
    /// - The progress bar style template is invalid
    /// - Multiple instances simultaneously write to the same shared progress output
    /// - The download futures report an internal thread failure
    ///
    #[allow(clippy::too_many_lines)]
    pub async fn download_dataset(
        self,
        label: Option<&str>,
        target_dir: PathBuf,
    ) -> color_eyre::Result<Self> {
        // make a new reqwest http client that can be shared between threads
        let shared_client = Client::new();

        // pull in the sets of files to be downloaded
        let dataset_files = self.get_downloads_per_dataset(label);

        // count the files to generate a message to inform the user of what will be downloaded
        let mut num_to_download = 0;
        for (_, files) in &dataset_files {
            num_to_download += files.len();
        }
        let message = if let Some(label_str) = label {
            format!("Downloading {num_to_download} files for project labeled '{label_str}'...")
        } else {
            format!("Downloading all {num_to_download} files listed in the refman registry...")
        };

        // Create a shared MultiProgress container.
        let mp = Arc::new(MultiProgress::new());

        // Create a top-level progress bar with total length equal to the number of files.
        let toplevel_pb = mp.add(ProgressBar::new(num_to_download as u64));
        toplevel_pb.set_style(
            ProgressStyle::default_bar()
                .template("{msg} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                .expect("Failed to set template"),
        );
        toplevel_pb.set_message(message);

        // put each download into its own tokio thread, and collect its handle into a vector
        // that can be polled downstream
        let mut dataset_task_handles: Vec<
            JoinHandle<Result<(RefDataset, MultiDownloadResults), ColorError>>,
        > = Vec::with_capacity(num_to_download);
        for (dataset, files) in dataset_files {
            let shared_client = shared_client.clone();
            let target_dir = target_dir.clone();
            let mp = mp.clone();

            // Spawn a task per dataset
            let handle: JoinHandle<_> = tokio::spawn(async move {
                // Inside this task: spawn parallel tasks for each file
                let file_task_handles = files.into_iter().map(|file| {
                    let client = shared_client.clone();
                    let dir = target_dir.clone();
                    let mp = mp.clone();

                    tokio::spawn(async move { request_dataset(file, client, &dir, mp).await })
                });

                // Await all file downloads for this dataset
                let file_results = try_join_all(file_task_handles).await?;

                Ok((dataset, file_results))
            });

            dataset_task_handles.push(handle);
        }

        let updated_datasets: Vec<RefDataset> = try_join_all(dataset_task_handles)
            .await?
            .into_iter()
            .filter_map(|dataset_result| {
                toplevel_pb.inc(1);
                match dataset_result {
                    Ok((dataset, file_results)) => {
                        match file_results.into_iter().collect::<Result<Vec<_>, _>>() {
                            Ok(successful_files) => Some((dataset, successful_files)),
                            Err(msg) => {
                                warn!("Failed to download files because of this error: {}", msg);
                                None
                            }
                        }
                    }
                    Err(msg) => {
                        warn!("Failed to download files because of this error: {}", msg);
                        None
                    }
                }
            })
            .flat_map(
                |(mut dataset, files)| -> Result<RefDataset, ValidationError> {
                    for file in files {
                        match file {
                            UnvalidatedFile::Fasta { .. } => {
                                let validated = file.try_validate()?;
                                let complete_download = DownloadStatus::new_downloaded(validated);
                                dataset.fasta = Some(complete_download);
                            }
                            UnvalidatedFile::Genbank { .. } => {
                                let validated = file.try_validate()?;
                                let complete_download = DownloadStatus::new_downloaded(validated);
                                dataset.genbank = Some(complete_download);
                            }
                            UnvalidatedFile::Gfa { .. } => {
                                let validated = file.try_validate()?;
                                let complete_download = DownloadStatus::new_downloaded(validated);
                                dataset.gfa = Some(complete_download);
                            }
                            UnvalidatedFile::Gff { .. } => {
                                let validated = file.try_validate()?;
                                let complete_download = DownloadStatus::new_downloaded(validated);
                                dataset.gff = Some(complete_download);
                            }
                            UnvalidatedFile::Gtf { .. } => {
                                let validated = file.try_validate()?;
                                let complete_download = DownloadStatus::new_downloaded(validated);
                                dataset.gtf = Some(complete_download);
                            }
                            UnvalidatedFile::Bed { .. } => {
                                let validated = file.try_validate()?;
                                let complete_download = DownloadStatus::new_downloaded(validated);
                                dataset.bed = Some(complete_download);
                            }
                        };
                    }
                    Ok(dataset)
                },
            )
            .collect();

        // Once all downloads finish, update and finish the overall progress bar.
        toplevel_pb.finish_with_message(format!(
            "Done! {num_to_download} files successfully downloaded to {target_dir:?}."
        ));

        // Update the project and return it
        let updated_project = self.update_registry(&updated_datasets);
        Ok(updated_project)
    }

    #[must_use]
    pub fn update_registry(self, new_datasets: &[RefDataset]) -> Project {
        // make a hashmap of the old datasets and new datasets we can compare for available updates
        let old_datasets: HashMap<&str, &RefDataset> = self
            .datasets()
            .iter()
            .map(|dataset| (dataset.label.as_str(), dataset))
            .collect();
        let updated_datasets: HashMap<&str, &RefDataset> = new_datasets
            .iter()
            .map(|dataset| (dataset.label.as_str(), dataset))
            .collect();

        // if a key in the old dataset is also in a new dataset, swap in the new data
        let merged_datasets: Vec<RefDataset> = old_datasets
            .into_iter()
            .map(|(label, dataset)| match updated_datasets.get(label) {
                Some(aha) => (*aha).to_owned(),
                None => dataset.clone(),
            })
            .collect();

        // use Rust's nice struct update syntax to create a new registry
        let updated_registry = Registry {
            datasets: merged_datasets,
            last_modified: Timestamp::now(),
            ..self.project
        };

        // return a new instance of the project in functional style
        Self {
            project: updated_registry,
        }
    }

    /// Removes a dataset from the Project's registry by its label.
    ///
    /// This method allows removing individual datasets from a refman Project's registry
    /// while maintaining the integrity of the remaining datasets. It can be used to:
    /// - Remove outdated or no longer needed reference datasets
    /// - Clean up the registry by removing temporary entries
    /// - Manage the project's dataset collection over time
    ///
    /// The method enforces several rules to maintain registry integrity:
    /// - The label must exactly match an existing dataset (case-sensitive)
    /// - The registry must maintain at least one dataset after removal
    /// - Only one dataset can be removed at a time
    ///
    /// This complements `register()` an`download_dataset()` in the lifecycle of managing
    /// reference data. While those methods add and fetch datasets, `remove()` allows
    /// pruning datasets that are no longer needed.
    ///
    /// # Arguments
    ///
    /// * `label` - The unique label identifying the dataset to remove from the registry
    ///
    /// # Returns
    ///
    /// Returns Ok(Project) with the updated Project if removal succeeds, or an
    /// `EntryError` in the following cases:
    /// - `EntryError::LabelNotFound` if no dataset matches the provided label
    /// - `EntryError::FinalEntry` if removing this dataset would empty the registry
    ///
    /// The Project instance is consumed and a new instance is returned to maintain
    /// the builder pattern used throughout the API.
    ///
    /// # Errors
    ///
    /// This method can return the following errors:
    /// - `EntryError::LabelNotFound` if the specified label is not in the registry
    /// - `EntryError::FinalEntry` if removing this dataset would empty the registry
    ///   entirely (at least one dataset must always remain)
    ///
    pub fn remove(mut self, label: &str) -> Result<Self, EntryError> {
        // make sure the label is in the recorded datasets
        if self
            .datasets()
            .iter()
            .filter(|dataset| dataset.label == label)
            .collect::<Vec<&RefDataset>>()
            .is_empty()
        {
            return Err(EntryError::LabelNotFound(label.to_string()));
        }

        // if it is, filter it out in place
        self.project
            .filter_datasets(|dataset| dataset.label != label);

        // return an error if that was the last entry
        if self.datasets().is_empty() {
            return Err(EntryError::FinalEntry(label.to_string()));
        }

        // otherwise, return the mutated project
        Ok(self)
    }

    fn print_single_label_data(self, label: &str) {
        let datasets = self.datasets();
        let dataset: Vec<_> = datasets
            .iter()
            .filter(|dataset| dataset.label == label)
            .collect();
        assert_eq!(
            dataset.len(),
            1,
            "No project with the label '{label}' has been registered. Run `refman list` without the label to see which datasets are registered."
        );
        let unwrapped_dataset = dataset[0];

        eprintln!("URLs registered for {label}:");
        eprintln!("--------------------{}", "-".repeat(label.len()));
        eprintln!(
            " - FASTA: {}",
            unwrapped_dataset
                .fasta
                .clone()
                .unwrap_or(DownloadStatus::default())
        );
        eprintln!(
            " - Genbank: {}",
            unwrapped_dataset
                .genbank
                .clone()
                .unwrap_or(DownloadStatus::default())
        );
        eprintln!(
            " - GFA: {}",
            unwrapped_dataset
                .gfa
                .clone()
                .unwrap_or(DownloadStatus::default())
        );
        eprintln!(
            " - GFF: {}",
            unwrapped_dataset
                .gff
                .clone()
                .unwrap_or(DownloadStatus::default())
        );
        eprintln!(
            " - GTF: {}",
            unwrapped_dataset
                .gtf
                .clone()
                .unwrap_or(DownloadStatus::default())
        );
        eprintln!(
            " - BED: {}",
            unwrapped_dataset
                .bed
                .clone()
                .unwrap_or(DownloadStatus::default())
        );
    }

    fn print_all_labels(self) {
        // print a title field if it has been set
        let title_field = &self.project.title;
        if let Some(title) = title_field {
            info!("Showing available data registered for {title}:");
        }

        // make a new mutable instance of a pretty table to be appended to
        let mut pretty_table = Table::new();

        // add the title row
        pretty_table.add_row(row![
            "Label", "FASTA", "Genbank", "GFA", "GFF", "GTF", "BED"
        ]);

        // add rows for each dataset
        let datasets = self.datasets();
        for dataset in datasets {
            pretty_table.add_row(row![
                dataset.label,
                abbreviate_str(
                    dataset
                        .fasta
                        .clone()
                        .unwrap_or(DownloadStatus::default())
                        .url_owned(),
                    20,
                    8,
                    25
                ),
                abbreviate_str(
                    dataset
                        .genbank
                        .clone()
                        .unwrap_or(DownloadStatus::default())
                        .url_owned(),
                    20,
                    8,
                    25
                ),
                abbreviate_str(
                    dataset
                        .gfa
                        .clone()
                        .unwrap_or(DownloadStatus::default())
                        .url_owned(),
                    20,
                    8,
                    25
                ),
                abbreviate_str(
                    dataset
                        .gff
                        .clone()
                        .unwrap_or(DownloadStatus::default())
                        .url_owned(),
                    20,
                    8,
                    25
                ),
                abbreviate_str(
                    dataset
                        .gtf
                        .clone()
                        .unwrap_or(DownloadStatus::default())
                        .url_owned(),
                    20,
                    8,
                    25
                ),
                abbreviate_str(
                    dataset
                        .bed
                        .clone()
                        .unwrap_or(DownloadStatus::default())
                        .url_owned(),
                    20,
                    8,
                    25
                ),
            ]);
        }

        pretty_table.printstd();
    }

    /// Pretty prints the currently registered datasets in a tabular format.
    ///
    /// This method provides a human-readable view of all reference datasets currently registered
    /// in the Project. It prints a formatted table showing each dataset's label and any
    /// registered file URLs for the supported bioinformatics formats (FASTA, Genbank, GFA,
    /// GFF, GTF, BED).
    ///
    /// The output is formatted as a table with columns for:
    /// - Dataset Label
    /// - FASTA URL (if registered)
    /// - Genbank URL (if registered)
    /// - GFA URL (if registered)
    /// - GFF URL (if registered)
    /// - GTF URL (if registered)
    /// - BED URL (if registered)
    ///
    /// Empty cells indicate that no URL is registered for that file format. If the Project
    /// has a title set, it will be displayed above the table.
    ///
    /// This provides an easy way to:
    /// - View all registered datasets at once
    /// - Check which file formats are available for each dataset
    /// - Verify dataset labels and URLs
    /// - Share the current state of your reference data registry
    ///
    /// The method consumes self as it follows the builder pattern used throughout the API.
    /// The actual printing is handled through the prettytable crate for consistent formatting.
    ///
    /// # Outputs
    ///
    /// Prints a formatted table to stdout. If the Project has a title, it is printed as a
    /// header above the table. Empty values in the table indicate no URL is registered for
    /// that format.
    ///
    /// # Notes
    ///
    /// The output is meant for human consumption and formatted for readability. For
    /// programmatic access to dataset information, use the `datasets()` or `datasets_owned()`
    /// methods instead.
    ///
    /// # Panics
    ///
    /// This method will panic if:
    /// - Multiple datasets with the same label exist in the registry when requesting a specific label
    /// - A requested dataset label does not exist when filtering registered datasets
    /// - The prettytable crate encounters an error when printing the output table
    pub fn prettyprint(self, label: Option<String>) {
        // if the user requested a label, just print the information for that label
        if let Some(label_str) = label {
            self.print_single_label_data(&label_str);
            return;
        }

        // otherwise, print all datasets as a table
        self.print_all_labels();
    }
}

#[inline]
fn abbreviate_str(s: String, max_chars: usize, head_chars: usize, tail_chars: usize) -> String {
    // Count the characters in the string.
    let char_count = s.chars().count();

    // If the string is not too long, return it unchanged.
    if char_count <= max_chars {
        return s;
    }

    // Collect the first `head_chars` characters.
    let head: String = s.chars().take(head_chars).collect();

    // Collect the last `tail_chars` characters.
    let tail: String = s
        .chars()
        .rev()
        .take(tail_chars)
        .collect::<String>()
        .chars()
        .rev()
        .collect();

    format!("{head}...{tail}")
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Registry {
    title: Option<String>,
    description: Option<String>,
    last_modified: Timestamp,
    global: bool,
    datasets: Vec<RefDataset>,
}

impl Default for Registry {
    fn default() -> Self {
        Registry {
            title: None,
            description: None,
            last_modified: Timestamp::now(),
            global: false,
            datasets: vec![],
        }
    }
}

impl Registry {
    fn filter_datasets<F>(&mut self, predicate: F)
    where
        F: FnMut(&RefDataset) -> bool,
    {
        self.datasets.retain(predicate);
    }
}

/// A configuration struct for customizing how refman interacts with registry files in your filesystem.
///
/// `RegistryOptions` is the primary way to control where and how refman stores its data. It provides
/// methods to:
/// - Set custom registry file locations
/// - Configure global vs local registry behavior
/// - Initialize new registry files
/// - Read from and write to existing registries
/// - Set project metadata like titles and descriptions
///
/// The struct resolves registry paths according to the following priority:
/// 1. User-specified custom path via `requested_path`
/// 2. For global registries (`global = true`):
///    - `$REFMAN_HOME/.refman/refman.toml` if `REFMAN_HOME` is set
///    - ~/.refman/refman.toml as default global location
/// 3. For local registries (`global = false`):
///    - ./refman.toml in current directory
///
/// This flexibility allows refman to support both project-specific local registries for individual
/// bioinformatics projects, as well as user-wide global registries for sharing reference data
/// between projects.
///
/// The struct maintains the resolved absolute path to the registry file, along with project
/// metadata and the global/local setting. It provides methods to safely initialize new registries
/// and read/write registry data while maintaining data integrity.
///
/// Generally you won't construct this struct directly, but rather obtain it through the Project
/// struct's methods which handle the configuration details automatically. However, advanced users
/// can use `RegistryOptions` directly for custom registry handling.
///
/// This is a core struct in refman's architecture, working closely with Project to provide the
/// foundational registry management capabilities that the rest of the tool builds upon.
pub struct RegistryOptions {
    resolved_path: PathBuf,
    title: Option<String>,
    description: Option<String>,
    global: bool,
}

impl RegistryOptions {
    /// Creates a new `RegistryOptions` instance with customized settings for registry file handling.
    ///
    /// This struct provides granular control over how refman interacts with registry files,
    /// determining where they are stored and how they are initialized. It implements the core
    /// logic for resolving registry paths according to the following priority:
    ///
    /// 1. User-specified custom path via `requested_path` parameter
    /// 2. For global registries (`global = true`):
    ///    - `$REFMAN_HOME/.refman/refman.toml` if `REFMAN_HOME` is set
    ///    - ~/.refman/refman.toml as default global location
    /// 3. For local registries (`global = false`):
    ///    - ./refman.toml in current directory
    ///
    /// The struct handles all filesystem interactions needed to:
    /// - Resolve and validate registry file paths
    /// - Create new registry files or directories as needed
    /// - Manage environment variables like `REFMAN_HOME`
    /// - Initialize registries with project metadata
    ///
    /// It works closely with the Project struct to provide the foundational registry
    /// management capabilities that refman builds upon. While most users will interact
    /// with registries through the Project API, this struct allows advanced users to
    /// customize registry behavior.
    ///
    /// The method performs validation to ensure the requested registry location is
    /// accessible and can be written to. It handles edge cases like missing directories
    /// and environment variables gracefully.
    ///
    /// # Arguments
    ///
    /// * `title` - Optional title for the registry/project
    /// * `description` - Optional description text
    /// * `requested_path` - Optional custom path where the registry should be stored
    /// * `global` - Whether this is a global (true) or local (false) registry
    ///
    /// # Returns
    ///
    /// Returns Ok(RegistryOptions) if initialization succeeds, or `RegistryError` if:
    /// - The requested path is invalid or inaccessible
    /// - Required directories cannot be created
    /// - Environment variables cannot be set
    /// - Other filesystem operations fail
    ///
    /// # Errors
    ///
    /// This method can return `RegistryError` variants for various filesystem and
    /// environment access failures. The error types provide context about what
    /// specifically failed during registry setup.
    pub fn try_new(
        title: Option<String>,
        description: Option<String>,
        requested_path: &Option<String>,
        global: bool,
    ) -> Result<RegistryOptions, RegistryError> {
        // If the user requested a path, see if it exists and is accessible, and
        // try to make it work
        if let Some(possible_path) = requested_path.as_deref() {
            let maybe_path = PathBuf::from_str(possible_path).ok();
            let resolved_path = resolve_registry_path(maybe_path, global)?;

            Ok(Self {
                resolved_path,
                title,
                description,
                global,
            })
        // otherwise, resolve a path with default settings
        } else {
            let resolved_path = resolve_registry_path(None, global)?;

            Ok(Self {
                resolved_path,
                title,
                description,
                global,
            })
        }
    }

    /// Initializes a new registry file for the Project if one doesn't already exist.
    ///
    /// This method handles creating and initializing the registry file that stores a
    /// Project's reference datasets and metadata. The registry file location is determined
    /// by the `RegistryOptions` configuration, following these rules:
    ///
    /// 1. User-specified custom path if provided to `RegistryOptions::try_new()`
    /// 2. For global registries (global = true):
    ///    - `$REFMAN_HOME/.refman/refman.toml` if `REFMAN_HOME` is set
    ///    - ~/.refman/refman.toml as default global location
    /// 3. For local registries (global = false):
    ///    - ./refman.toml in current directory
    ///
    /// The method will:
    /// - Create a new refman.toml file if one doesn't exist at the resolved path
    /// - Initialize it with provided title and description if specified
    /// - Set appropriate global/local flag
    /// - Create any necessary parent directories
    /// - Handle filesystem permissions and access
    ///
    /// If a registry file already exists at the target location, the method will
    /// log an informational message and take no action, preserving the existing
    /// registry data.
    ///
    /// This is typically called automatically when creating new Projects, but can
    /// be called directly for custom registry initialization workflows. The method
    /// integrates with refman's overall registry management system to maintain
    /// data integrity and consistent state.
    ///
    /// # Returns
    ///
    /// Returns Ok(()) if initialization succeeds or registry already exists.
    /// Returns `RegistryError` if filesystem operations fail due to permissions,
    /// invalid paths, or other IO errors.
    ///
    /// # Errors
    ///
    /// Can return `RegistryError` variants for:
    /// - Failed file creation
    /// - Invalid paths
    /// - Insufficient permissions
    /// - Filesystem errors
    pub fn init(&self) -> Result<(), RegistryError> {
        // If a refman.toml doesn't exist, make it and write out the available information
        if self.resolved_path.exists() {
            info!("A refman registry already exists. Start filling it with `refman register`.");
        } else {
            let mut new_project =
                Project::new(self.title.clone(), self.description.clone(), self.global);
            File::create(&self.resolved_path)?;

            self.write_registry(&mut new_project)?;
            // Otherwise, do nothing except log out that a registry file already exists
        }
        Ok(())
    }

    /// Reads and deserializes a registry file into a Project, or initializes a new empty Project.
    ///
    /// This method handles loading registry data from refman.toml files. It follows these rules:
    /// - If no registry file exists at the resolved path, returns a default empty Project
    /// - If an empty registry file exists, returns a default empty Project
    /// - Otherwise deserializes the TOML file into a Project instance
    ///
    /// The registry file path is determined by `RegistryOptions` rules, in order:
    /// 1. User-specified custom path if provided
    /// 2. For global registries (global = true):
    ///    - `$REFMAN_HOME/.refman/refman.toml`
    ///    - ~/.refman/refman.toml (default)
    /// 3. For local registries (global = false):
    ///    - ./refman.toml
    ///
    /// This method is core to refman's persistence layer, allowing Projects to be saved and
    /// loaded across sessions. It works in tandem with `write_registry()` to maintain registry
    /// state. The registry files store:
    /// - Project metadata (title, description)
    /// - Dataset entries with labels and file URLs
    /// - Last modified timestamp
    /// - Global/local status
    ///
    /// The method handles common edge cases like:
    /// - Missing registry files
    /// - Empty registry files
    /// - Invalid TOML formatting
    /// - File access errors
    ///
    /// This is typically called internally by Project methods that need to load registry
    /// state, but can be used directly for custom registry reading workflows.
    ///
    /// # Returns
    ///
    /// Returns Ok(Project) containing either:
    /// - A deserialized Project from the registry file
    /// - A new empty Project if no valid registry exists
    ///
    /// # Errors
    ///
    /// Returns `RegistryError` if:
    /// - File operations fail (permissions, IO errors)
    /// - TOML deserialization fails
    /// - Registry path resolution fails
    pub fn read_registry(&self) -> Result<Project, RegistryError> {
        // To save some effort, first check if the refman.toml exists. If it doesn't,
        // just set up a project with default settings and early-return that
        if !self.resolved_path.exists() {
            let new_project = Project::default();
            return Ok(new_project);
        }

        // Additionally, if a file exists but is empty, pretend it doesn't exist and do
        // the same thing as above
        if std::fs::metadata(&self.resolved_path)?.len() == 0 {
            let new_project = Project::default();
            return Ok(new_project);
        }

        // If neither of those conditions were met, read and deserialize the TOML
        // file into a Project struct and return it
        let toml_contents = read_to_string(self.resolved_path.clone())?;
        let project: Project = toml::from_str(&toml_contents)?;
        Ok(project)
    }
    /// Writes a Project's registry data to the refman.toml file at the resolved registry path.
    ///
    /// This method handles persisting Project state to disk, including:
    /// - All registered datasets with their labels and file URLs
    /// - Project metadata like title and description
    /// - Last modified timestamp
    /// - Global/local registry status
    ///
    /// The registry file location follows `RegistryOptions` rules, in order:
    /// 1. User-specified custom path if provided
    /// 2. For global registries (global = true):
    ///    - `$REFMAN_HOME/.refman/refman.toml`
    ///    - ~/.refman/refman.toml (default)
    /// 3. For local registries (global = false):
    ///    - ./refman.toml
    ///
    /// This method works in tandem with `read_registry()` to maintain persistent state
    /// across refman sessions. When writing, it:
    /// - Updates the last modified timestamp
    /// - Serializes the Project data to TOML format
    /// - Writes the TOML to the resolved registry path
    /// - Creates/overwrites the registry file as needed
    ///
    /// This is typically called internally by Project methods that modify state, but
    /// can be used directly for custom registry writing workflows. The method integrates
    /// with refman's overall registry management system to maintain data integrity.
    ///
    /// # Arguments
    ///
    /// * `project` - Mutable reference to the Project whose state should be written
    ///
    /// # Returns
    ///
    /// Returns Ok(()) if the write succeeds, or `RegistryError` if filesystem operations fail.
    ///
    /// # Errors
    ///
    /// Returns `RegistryError` if:
    /// - File operations fail (permissions, IO errors)
    /// - TOML serialization fails
    /// - Registry path resolution fails
    ///
    /// # Panics
    ///
    /// This method does not panic under normal circumstances, but may panic if the filesystem
    /// becomes inaccessible while writing or if memory allocation fails during serialization.
    pub fn write_registry(&self, project: &mut Project) -> Result<(), RegistryError> {
        // update the timestamp
        project.project.last_modified = Timestamp::now();

        // serialize and write out the TOML file
        let toml_text = toml::to_string_pretty(project)?;
        fs::write(&self.resolved_path, toml_text)?;

        Ok(())
    }
}

fn resolve_registry_path(
    maybe_path: Option<PathBuf>,
    global: bool,
) -> Result<PathBuf, RegistryError> {
    // to resolve a registry path, a fair amount of control flow needs to happen to unwrap a few conditions.
    // First, we prioritize a directory the user requests we place the registry in, if provided. This is the simplest
    // branch and comes first.
    let registry_path = match maybe_path {
        Some(valid_path) => {
            if let Some(path_str) = valid_path.to_str() {
                debug!("Setting the refman home to '{path_str}'");
                set_refman_home(path_str);
            }
            valid_path.join("refman.toml")
        }

        // If the user did not request a particular directory, we then check if a global registry was requested.
        // If not, this is the next simplest case; just place the registry in the current working directory (ideally,
        // the project root).
        None => {
            // If not global, use the current directory as the refman home and return the full path.
            if !global {
                let current_dir = current_dir()?;
                if let Some(current_dir_string) = current_dir.to_str() {
                    debug!("Setting the refman home to '{current_dir_string}'");
                    set_refman_home(current_dir_string);
                };

                return Ok(current_dir.join("refman.toml"));
            }

            // If no desired directory was provided, but the user also requested that the registry is global, first
            // check the environment variable REFMAN_HOME for the registry's location.
            let refman_home: Option<PathBuf> = match env::var("REFMAN_HOME") {
                Ok(path_str) => {
                    debug!(
                        "Desired file path detected in the REFMAN_HOME environment variable: '{}'. A global registry will be placed there.",
                        path_str
                    );
                    let path = PathBuf::from(path_str);
                    Some(path)
                }
                // If that environment variable isn't set, place it in the home directory.
                Err(_) => {
                    debug!(
                        "The REFMAN_HOME variable is not set. The registry will thus be placed in its default location in the user's home directory."
                    );
                    dirs::home_dir()
                }
            };

            // Finally, whether the home directory is being used or the current directory as a fallback, join on
            // a subdirectory called ".refman" and then "refman.toml" onto that.
            if let Some(dir) = refman_home {
                let resolved_home = dir.join(".refman");
                debug!("setting the refman home to '{:?}'", resolved_home);
                resolved_home
            } else {
                warn!("unable to access home directory, so `refman `will place its registry in the current working directory. unless this path is provided in the next `refman` run, `refman` may be unable to pick up where it leaves off during the current run.");
                let current_dir = current_dir()?;
                if let Some(current_dir_string) = current_dir.to_str() {
                    debug!("setting the refman home to '{current_dir_string}'");
                    set_refman_home(current_dir_string);
                };
                let resolved_home = current_dir.join(".refman");
                debug!("setting the refman home to '{:?}'", resolved_home);
                resolved_home
            }.join("refman.toml")
        } // TODO: Eventually, it would be cool to have a global dotfile config for refman so the user doesn't have
          // to tell it to operate globally every time.
    };

    Ok(registry_path)
}

fn set_refman_home(desired_dir: &str) {
    // If REFMAN_HOME is set,
    if let Ok(old_home) = env::var("REFMAN_HOME") {
        warn!(
            "The environment variable $REFMAN_HOME was previously set to {}, but a new location at {} was requested. `refman` will overwrite the old $REFMAN_HOME value and proceed.",
            old_home, desired_dir
        );
        unsafe { env::set_var("REFMAN_HOME", desired_dir) }
    } else {
        debug!(
            "The REFMAN_HOME environment variable has not previously been set. Now setting it to the requested directory, {}",
            desired_dir
        );
        unsafe { env::set_var("REFMAN_HOME", desired_dir) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_new_project() {
        let title = Some("Test Project".to_string());
        let desc = Some("A test project".to_string());
        let project = Project::new(title.clone(), desc.clone(), false);

        assert_eq!(project.project.title, title);
        assert_eq!(project.project.description, desc);
        assert!(!project.project.global);
        assert!(project.project.datasets.is_empty());
    }

    #[test]
    fn test_is_registered() {
        let mut project = Project::new(None, None, false);
        let dataset = RefDataset {
            label: "test_genome".into(),
            fasta: Some(DownloadStatus::new(
                "https://example.com/genome.fasta".to_string(),
            )),
            ..Default::default()
        };

        project = project.register(dataset).unwrap();
        assert!(project.is_registered("test_genome"));
        assert!(!project.is_registered("nonexistent"));
    }

    #[test]
    fn test_registry_options_new() {
        let temp_dir = tempdir().unwrap();
        let dir_path = temp_dir.path().to_str().unwrap();

        let options = RegistryOptions::try_new(
            Some("Test Registry".to_string()),
            Some("Test Description".to_string()),
            &Some(dir_path.to_string()),
            false,
        )
        .unwrap();

        assert_eq!(
            options.resolved_path,
            PathBuf::from(dir_path).join("refman.toml")
        );
        assert_eq!(options.title, Some("Test Registry".to_string()));
        assert_eq!(options.description, Some("Test Description".to_string()));
        assert!(!options.global);
    }

    #[test]
    fn test_read_write_registry() {
        let temp_dir = tempdir().unwrap();
        let dir_path = temp_dir.path().to_str().unwrap();

        let options =
            RegistryOptions::try_new(None, None, &Some(dir_path.to_string()), false).unwrap();

        // Test writing
        let mut project = Project::new(None, None, false);
        options.write_registry(&mut project).unwrap();
        assert!(options.resolved_path.exists());

        // Test reading
        let read_project = options.read_registry().unwrap();
        assert_eq!(read_project.datasets().len(), 0);
    }
}
