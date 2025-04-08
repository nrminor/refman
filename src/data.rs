use std::{
    fmt::Display,
    path::{Path, PathBuf},
};

use log::debug;
use serde::{Deserialize, Serialize};

use crate::{
    EntryError, ValidationError,
    downloads::check_url,
    validate::{UnvalidatedFile, ValidatedFile, hash_valid_download},
};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum DownloadStatus {
    NotYetDownloaded(String),
    Downloaded(ValidatedFile),
}

impl Default for DownloadStatus {
    fn default() -> Self {
        DownloadStatus::NotYetDownloaded(String::new())
    }
}

impl Display for DownloadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadStatus::NotYetDownloaded(undownloaded) => {
                write!(f, "NotYetDownloaded: {undownloaded}")
            }
            DownloadStatus::Downloaded(validated_file) => {
                write!(f, "Downloaded: {validated_file}")
            }
        }
    }
}

impl DownloadStatus {
    #[must_use]
    pub fn new(file: String) -> Self {
        DownloadStatus::NotYetDownloaded(file)
    }

    #[must_use]
    pub fn new_downloaded(file: ValidatedFile) -> Self {
        Self::Downloaded(file)
    }

    #[must_use]
    pub fn url(&self) -> &str {
        match self {
            DownloadStatus::NotYetDownloaded(url) => url,
            DownloadStatus::Downloaded(validated_file) => &validated_file.uri,
        }
    }

    #[must_use]
    pub fn url_owned(&self) -> String {
        match self {
            DownloadStatus::NotYetDownloaded(url) => url.to_owned(),
            DownloadStatus::Downloaded(validated_file) => validated_file.uri.clone(),
        }
    }

    #[must_use]
    pub fn is_downloaded(&self) -> bool {
        match self {
            DownloadStatus::NotYetDownloaded(_) => false,
            DownloadStatus::Downloaded(_) => true,
        }
    }

    #[must_use]
    pub fn is_validated(&self) -> bool {
        match self {
            DownloadStatus::NotYetDownloaded(_) => false,
            DownloadStatus::Downloaded(validated_file) => validated_file.validated,
        }
    }
}

/// A structure that manages various types of data associated with a single biological reference dataset.
/// A reference dataset typically consists of sequence files (like FASTA or Genbank)
/// and optional annotation files (like GFF, GTF, or BED) that provide additional layers of genomic
/// information.
///
/// The structure enforces important data integrity rules:
/// - Every dataset must have a unique label for identification
/// - At least one file must be associated with a label
/// - Annotation files (GFF, GTF, BED) can only be included if there's an associated sequence file
///   (FASTA or Genbank) present
///
/// Each field represents a different file format commonly used in bioinformatics:
/// - FASTA: Raw sequence data
/// - Genbank: Annotated sequence data
/// - GFA: Genome/gene assembly graphs
/// - GFF: General Feature Format for genomic features
/// - GTF: Gene Transfer Format (a refined version of GFF)
/// - BED: Browser Extensible Data format for genomic intervals
///
/// Files are stored as optional strings, typically representing paths or identifiers to the actual
/// data. This allows for flexible dataset configurations while maintaining data integrity through
/// the `try_new()` constructor.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct RefDataset {
    pub label: String,
    // TODO: Replace the strings with the `DownloadStatus` enum
    pub fasta: Option<DownloadStatus>,
    pub genbank: Option<DownloadStatus>,
    pub gfa: Option<DownloadStatus>,
    pub gff: Option<DownloadStatus>,
    pub gtf: Option<DownloadStatus>,
    pub bed: Option<DownloadStatus>,
}

impl RefDataset {
    /// Create a new reference dataset while enforcing data integrity rules.
    ///
    /// This method creates a new [`RefDataset`] instance after validating that certain
    /// critical invariants are maintained:
    /// - Every dataset must have a non-empty label for identification
    /// - At least one file (FASTA, Genbank, GFA, GFF, GTF, or BED) must be associated with a label
    /// - Annotation files (GFF, GTF, BED) can only be included if there's an associated sequence file
    ///   (FASTA or Genbank) present
    /// - All provided file URLs must be valid and accessible
    ///
    /// # Arguments
    ///
    /// * `label` - A unique identifier for this reference dataset
    /// * `fasta` - Optional URL to a FASTA format sequence file
    /// * `genbank` - Optional URL to a Genbank format sequence file
    /// * `gfa` - Optional URL to a GFA format assembly graph file
    /// * `gff` - Optional URL to a GFF format annotation file
    /// * `gtf` - Optional URL to a GTF format annotation file
    /// * `bed` - Optional URL to a BED format annotation file
    ///
    /// # Returns
    ///
    /// Returns a `Result<RefDataset, EntryError>` which is:
    /// - `Ok(RefDataset)` if all validation passes
    /// - `Err(EntryError)` if any validation fails
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - No files are provided with the label (`EntryError::LabelButNoFiles`)
    /// - Annotation files are provided without sequence files (`EntryError::AnnotationsButNoSequence`)
    /// - Any provided URL is invalid or inaccessible
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use your_crate::RefDataset;
    ///
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let dataset = RefDataset::try_new(
    ///     "hg38".to_string(),
    ///     Some("https://example.com/hg38.fa".to_string()),
    ///     None,
    ///     None,
    ///     Some("https://example.com/hg38.gff".to_string()),
    ///     None,
    ///     None
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::similar_names)]
    pub async fn try_new(
        label: String,
        fasta: Option<String>,
        genbank: Option<String>,
        gfa: Option<String>,
        gff: Option<String>,
        gtf: Option<String>,
        bed: Option<String>,
    ) -> Result<Self, EntryError> {
        match (&fasta, &genbank, &gff, &gtf, &bed) {
            // This is the case when no files are provided, but a label is (label is the only argument to this function
            // that is not an Option<String>)
            (None, None, None, None, None) => Err(EntryError::LabelButNoFiles),

            // The following cases occur when annotation file(s) are registered without a sequence file, e.g., FASTA or
            // Genbank, to pull from/associate with.
            (None, None, None, None, Some(label))
            | (None, None, None, Some(label), None | Some(_))
            | (None, None, Some(label), None | Some(_), None | Some(_)) => {
                Err(EntryError::AnnotationsButNoSequence(label.to_string()))
            }

            // If none of the above conditions are met, we're all good! Return an instance of the `RefDataset` struct
            // with validated combinations of fields.
            _ => {
                // check each of the possible files, if provided by the user. If all are successful, initialize each
                // file name wrapped in a `DownloadStatus` `NotYetDownloaded` variant, which preserves backwards
                // compatibility with the `refman.toml` format and controls the valid ways state can be updated in the
                // `refman` register-download-validate workflow. We'll just use variable shadowing here instead of
                // binding new variables.
                let fasta = if let Some(url_to_check) = fasta {
                    let _ = check_url(&url_to_check).await?;
                    let status = DownloadStatus::new(url_to_check);
                    Some(status)
                } else {
                    None
                };
                let genbank = if let Some(url_to_check) = genbank {
                    let _ = check_url(&url_to_check).await?;
                    let status = DownloadStatus::new(url_to_check);
                    Some(status)
                } else {
                    None
                };
                let gfa = if let Some(url_to_check) = gfa {
                    let _ = check_url(&url_to_check).await?;
                    let status = DownloadStatus::new(url_to_check);
                    Some(status)
                } else {
                    None
                };
                let gff = if let Some(url_to_check) = gff {
                    let _ = check_url(&url_to_check).await?;
                    let status = DownloadStatus::new(url_to_check);
                    Some(status)
                } else {
                    None
                };
                let gtf = if let Some(url_to_check) = gtf {
                    let _ = check_url(&url_to_check).await?;
                    let status = DownloadStatus::new(url_to_check);
                    Some(status)
                } else {
                    None
                };
                let bed = if let Some(url_to_check) = bed {
                    let _ = check_url(&url_to_check).await?;
                    let status = DownloadStatus::new(url_to_check);
                    Some(status)
                } else {
                    None
                };

                // If all provided URLs are valid, set up an instance of a registry
                Ok(Self {
                    label,
                    fasta,
                    genbank,
                    gfa,
                    gff,
                    gtf,
                    bed,
                })
            }
        }
    }

    pub(crate) fn get_fasta_download(&self, target_dir: &Path) -> Option<UnvalidatedFile> {
        // resolve state for each of the files
        match &self.fasta {
            Some(file) => match file {
                DownloadStatus::NotYetDownloaded(uri) => {
                    let unvalidated = UnvalidatedFile::Fasta {
                        uri: uri.clone(),
                        local_path: PathBuf::new(),
                    };
                    Some(unvalidated)
                }
                DownloadStatus::Downloaded(validated_file) => {
                    debug!(
                        "Deciding whether to re-download the previously downloaded file at {:?}...",
                        validated_file
                    );

                    // pull in the previously downloaded file path
                    let old_path = &validated_file.local_path;

                    // make sure the old file still exists or is in the requested destination. If not, it should
                    // be downloaded.
                    if !old_path.exists() || !old_path.starts_with(target_dir) {
                        return Some(UnvalidatedFile::Fasta {
                            uri: validated_file.uri.clone(),
                            local_path: PathBuf::new(),
                        });
                    }

                    // make sure there's a hash we can use to checksum
                    let Some(old_hash) = &validated_file.hash else {
                        debug!("The file was never hashed, so it will be re-downloaded");
                        return None;
                    };

                    // make sure the file exists and still matches the hash. Otherwise, re-download.
                    let Ok(new_hash) = hash_valid_download(old_path) else {
                        debug!(
                            "The checksum failed because the file could not be accessed, so it will be redownloaded"
                        );
                        return None;
                    };
                    if old_path.exists() && old_hash.eq(&new_hash) {
                        debug!(
                            "The path previously recorded for the download, {:?}, existed and it passed the checksum, so it will not be re-downloaded",
                            old_path,
                        );
                        return None;
                    }

                    // if we've made it this far, the file should be redownloaded. Clear the
                    // local path and fill the URI into an UnvalidatedFile variant
                    let unvalidated = UnvalidatedFile::Fasta {
                        uri: validated_file.uri.clone(),
                        local_path: PathBuf::new(),
                    };
                    Some(unvalidated)
                }
            },
            None => None,
        }
    }

    pub(crate) fn get_genbank_download(&self, target_dir: &Path) -> Option<UnvalidatedFile> {
        match &self.genbank {
            Some(file) => match file {
                DownloadStatus::NotYetDownloaded(uri) => {
                    let unvalidated = UnvalidatedFile::Genbank {
                        uri: uri.to_string(),
                        local_path: PathBuf::new(),
                    };
                    Some(unvalidated)
                }
                DownloadStatus::Downloaded(validated_file) => {
                    debug!(
                        "Deciding whether to re-download the previously downloaded file at {:?}...",
                        validated_file
                    );

                    // pull in the previously downloaded file path
                    let old_path = &validated_file.local_path;

                    // make sure the old file still exists. If not, it should be downloaded.
                    if !old_path.exists() || !old_path.starts_with(target_dir) {
                        return Some(UnvalidatedFile::Genbank {
                            uri: validated_file.uri.clone(),
                            local_path: PathBuf::new(),
                        });
                    }

                    // make sure there's a hash we can use to checksum
                    let Some(old_hash) = &validated_file.hash else {
                        debug!("The file was never hashed, so it will be re-downloaded");
                        return None;
                    };

                    // make sure the file exists and still matches the hash. Otherwise, re-download.
                    let Ok(new_hash) = hash_valid_download(old_path) else {
                        debug!(
                            "The checksum failed because the file could not be accessed, so it will be redownloaded"
                        );
                        return None;
                    };
                    if old_path.exists() && old_hash.eq(&new_hash) {
                        debug!(
                            "The path previously recorded for the download, {:?}, existed and it passed the checksum, so it will not be re-downloaded",
                            old_path,
                        );
                        return None;
                    }

                    // if we've made it this far, the file should be redownloaded. Clear the
                    // local path and fill the URI into an UnvalidatedFile variant
                    let unvalidated = UnvalidatedFile::Genbank {
                        uri: validated_file.uri.clone(),
                        local_path: PathBuf::new(),
                    };
                    Some(unvalidated)
                }
            },
            None => None,
        }
    }

    pub(crate) fn get_gfa_download(&self, target_dir: &Path) -> Option<UnvalidatedFile> {
        match &self.gfa {
            Some(file) => match file {
                DownloadStatus::NotYetDownloaded(uri) => {
                    let unvalidated = UnvalidatedFile::Gfa {
                        uri: uri.to_string(),
                        local_path: PathBuf::new(),
                    };
                    Some(unvalidated)
                }
                DownloadStatus::Downloaded(validated_file) => {
                    debug!(
                        "Deciding whether to re-download the previously downloaded file at {:?}...",
                        validated_file
                    );

                    // pull in the previously downloaded file path
                    let old_path = &validated_file.local_path;

                    // make sure the old file still exists. If not, it should be downloaded.
                    if !old_path.exists() || !old_path.starts_with(target_dir) {
                        return Some(UnvalidatedFile::Gfa {
                            uri: validated_file.uri.clone(),
                            local_path: PathBuf::new(),
                        });
                    }

                    // make sure there's a hash we can use to checksum
                    let Some(old_hash) = &validated_file.hash else {
                        debug!("The file was never hashed, so it will be re-downloaded");
                        return None;
                    };

                    // make sure the file exists and still matches the hash. Otherwise, re-download.
                    let Ok(new_hash) = hash_valid_download(old_path) else {
                        debug!(
                            "The checksum failed because the file could not be accessed, so it will be redownloaded"
                        );
                        return None;
                    };
                    if old_path.exists() && old_hash.eq(&new_hash) {
                        debug!(
                            "The path previously recorded for the download, {:?}, existed and it passed the checksum, so it will not be re-downloaded",
                            old_path,
                        );
                        return None;
                    }

                    // if we've made it this far, the file should be redownloaded. Clear the
                    // local path and fill the URI into an UnvalidatedFile variant
                    let unvalidated = UnvalidatedFile::Gfa {
                        uri: validated_file.uri.clone(),
                        local_path: PathBuf::new(),
                    };
                    Some(unvalidated)
                }
            },
            None => None,
        }
    }

    pub(crate) fn get_gff_download(&self, target_dir: &Path) -> Option<UnvalidatedFile> {
        match &self.gff {
            Some(file) => match file {
                DownloadStatus::NotYetDownloaded(uri) => {
                    let unvalidated = UnvalidatedFile::Gff {
                        uri: uri.to_string(),
                        local_path: PathBuf::new(),
                    };
                    Some(unvalidated)
                }
                DownloadStatus::Downloaded(validated_file) => {
                    debug!(
                        "Deciding whether to re-download the previously downloaded file at {:?}...",
                        validated_file
                    );

                    // pull in the previously downloaded file path
                    let old_path = &validated_file.local_path;

                    // make sure the old file still exists. If not, it should be downloaded.
                    if !old_path.exists() || !old_path.starts_with(target_dir) {
                        return Some(UnvalidatedFile::Gff {
                            uri: validated_file.uri.clone(),
                            local_path: PathBuf::new(),
                        });
                    }

                    // make sure there's a hash we can use to checksum
                    let Some(old_hash) = &validated_file.hash else {
                        debug!("The file was never hashed, so it will be re-downloaded");
                        return None;
                    };

                    // make sure the file exists and still matches the hash. Otherwise, re-download.
                    let Ok(new_hash) = hash_valid_download(old_path) else {
                        debug!(
                            "The checksum failed because the file could not be accessed, so it will be redownloaded"
                        );
                        return None;
                    };
                    if old_path.exists() && old_hash.eq(&new_hash) {
                        debug!(
                            "The path previously recorded for the download, {:?}, existed and it passed the checksum, so it will not be re-downloaded",
                            old_path,
                        );
                        return None;
                    }

                    // if we've made it this far, the file should be redownloaded. Clear the
                    // local path and fill the URI into an UnvalidatedFile variant
                    let unvalidated = UnvalidatedFile::Gff {
                        uri: validated_file.uri.clone(),
                        local_path: PathBuf::new(),
                    };
                    Some(unvalidated)
                }
            },
            None => None,
        }
    }

    pub(crate) fn get_gtf_download(&self, target_dir: &Path) -> Option<UnvalidatedFile> {
        match &self.gtf {
            Some(file) => match file {
                DownloadStatus::NotYetDownloaded(uri) => {
                    let unvalidated = UnvalidatedFile::Gtf {
                        uri: uri.to_string(),
                        local_path: PathBuf::new(),
                    };
                    Some(unvalidated)
                }
                DownloadStatus::Downloaded(validated_file) => {
                    debug!(
                        "Deciding whether to re-download the previously downloaded file at {:?}...",
                        validated_file
                    );

                    // pull in the previously downloaded file path
                    let old_path = &validated_file.local_path;

                    // make sure the old file still exists. If not, it should be downloaded.
                    if !old_path.exists() || !old_path.starts_with(target_dir) {
                        return Some(UnvalidatedFile::Gtf {
                            uri: validated_file.uri.clone(),
                            local_path: PathBuf::new(),
                        });
                    }

                    // make sure there's a hash we can use to checksum
                    let Some(old_hash) = &validated_file.hash else {
                        debug!("The file was never hashed, so it will be re-downloaded");
                        return None;
                    };

                    // make sure the file exists and still matches the hash. Otherwise, re-download.
                    let Ok(new_hash) = hash_valid_download(old_path) else {
                        debug!(
                            "The checksum failed because the file could not be accessed, so it will be redownloaded"
                        );
                        return None;
                    };
                    if old_path.exists() && old_hash.eq(&new_hash) {
                        debug!(
                            "The path previously recorded for the download, {:?}, existed and it passed the checksum, so it will not be re-downloaded",
                            old_path,
                        );
                        return None;
                    }

                    // if we've made it this far, the file should be redownloaded. Clear the
                    // local path and fill the URI into an UnvalidatedFile variant
                    let unvalidated = UnvalidatedFile::Gtf {
                        uri: validated_file.uri.clone(),
                        local_path: PathBuf::new(),
                    };
                    Some(unvalidated)
                }
            },
            None => None,
        }
    }

    pub(crate) fn get_bed_download(&self, target_dir: &Path) -> Option<UnvalidatedFile> {
        match &self.bed {
            Some(file) => match file {
                DownloadStatus::NotYetDownloaded(uri) => {
                    let unvalidated = UnvalidatedFile::Bed {
                        uri: uri.to_string(),
                        local_path: PathBuf::new(),
                    };
                    Some(unvalidated)
                }
                DownloadStatus::Downloaded(validated_file) => {
                    debug!(
                        "Deciding whether to re-download the previously downloaded file at {:?}...",
                        validated_file
                    );

                    // pull in the previously downloaded file path
                    let old_path = &validated_file.local_path;

                    // make sure the old file still exists. If not, it should be downloaded.
                    if !old_path.exists() || !old_path.starts_with(target_dir) {
                        return Some(UnvalidatedFile::Bed {
                            uri: validated_file.uri.clone(),
                            local_path: PathBuf::new(),
                        });
                    }

                    // make sure there's a hash we can use to checksum
                    let Some(old_hash) = &validated_file.hash else {
                        debug!("The file was never hashed, so it will be re-downloaded");
                        return None;
                    };

                    // make sure the file exists and still matches the hash. Otherwise, re-download.
                    let Ok(new_hash) = hash_valid_download(old_path) else {
                        debug!(
                            "The checksum failed because the file could not be accessed, so it will be redownloaded"
                        );
                        return None;
                    };
                    if old_path.exists() && old_hash.eq(&new_hash) {
                        debug!(
                            "The path previously recorded for the download, {:?}, existed and it passed the checksum, so it will not be re-downloaded",
                            old_path,
                        );
                        return None;
                    }

                    // if we've made it this far, the file should be redownloaded. Clear the
                    // local path and fill the URI into an UnvalidatedFile variant
                    let unvalidated = UnvalidatedFile::Bed {
                        uri: validated_file.uri.clone(),
                        local_path: PathBuf::new(),
                    };
                    Some(unvalidated)
                }
            },
            None => None,
        }
    }

    /// Updates the state of the dataset with newly downloaded and validated file's information.
    ///
    /// This method takes an `UnvalidatedFile` that has been downloaded and validates it,
    /// updating the corresponding field in the dataset with the new download status. This
    /// is a key part of the refman register-download-validate workflow, transitioning files
    /// from the `NotYetDownloaded` to `Downloaded` state.
    ///
    /// The method handles all supported file types (FASTA, Genbank, GFA, GFF, GTF, BED)
    /// and updates the respective field in the dataset with validated file information,
    /// including hash values and local paths.
    ///
    /// # Arguments
    ///
    /// * `downloaded_file` - An `UnvalidatedFile` containing the downloaded file's information,
    ///    including its URI and local path
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if validation and update succeeds, otherwise returns a `ValidationError`
    ///
    /// # Errors
    ///
    /// Returns a `ValidationError` if:
    /// - The file fails validation checks
    /// - The file hash cannot be computed
    /// - The file type is invalid or corrupted
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use your_crate::{RefDataset, UnvalidatedFile};
    /// use std::path::PathBuf;
    ///
    /// let mut dataset = RefDataset::default();
    /// let downloaded = UnvalidatedFile::Fasta {
    ///     uri: "https://example.com/file.fa".to_string(),
    ///     local_path: PathBuf::from("/tmp/file.fa"),
    /// };
    /// dataset.update_with_download(&downloaded).unwrap();
    /// ```
    pub fn update_with_download(
        &mut self,
        downloaded_file: &UnvalidatedFile,
    ) -> Result<(), ValidationError> {
        match downloaded_file {
            UnvalidatedFile::Fasta { .. } => {
                let validated = downloaded_file.try_validate()?;
                let updated_status = DownloadStatus::new_downloaded(validated);

                self.fasta = Some(updated_status);
            }
            UnvalidatedFile::Genbank { .. } => {
                let validated = downloaded_file.try_validate()?;
                let updated_status = DownloadStatus::new_downloaded(validated);

                self.genbank = Some(updated_status);
            }
            UnvalidatedFile::Gfa { .. } => {
                let validated = downloaded_file.try_validate()?;
                let updated_status = DownloadStatus::new_downloaded(validated);

                self.gfa = Some(updated_status);
            }
            UnvalidatedFile::Gff { .. } => {
                let validated = downloaded_file.try_validate()?;
                let updated_status = DownloadStatus::new_downloaded(validated);

                self.gff = Some(updated_status);
            }
            UnvalidatedFile::Gtf { .. } => {
                let validated = downloaded_file.try_validate()?;
                let updated_status = DownloadStatus::new_downloaded(validated);

                self.gtf = Some(updated_status);
            }
            UnvalidatedFile::Bed { .. } => {
                let validated = downloaded_file.try_validate()?;
                let updated_status = DownloadStatus::new_downloaded(validated);

                self.bed = Some(updated_status);
            }
        }

        Ok(())
    }
}
