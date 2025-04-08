use flate2::read::GzDecoder;
use jiff::Timestamp;
use md5::{Context, Digest};
use noodles::{bed, fasta, gff, gtf};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::{
    fmt::Display,
    fs::File,
    io::{BufReader, Read},
    path::{Path, PathBuf},
};

use crate::{RefDataset, ValidationError, data::DownloadStatus};

#[derive(Debug)]
pub enum UnvalidatedFile {
    Fasta { uri: String, local_path: PathBuf },
    Genbank { uri: String, local_path: PathBuf },
    Gfa { uri: String, local_path: PathBuf },
    Gff { uri: String, local_path: PathBuf },
    Gtf { uri: String, local_path: PathBuf },
    Bed { uri: String, local_path: PathBuf },
}

impl UnvalidatedFile {
    pub(crate) fn url(&self) -> &str {
        match self {
            UnvalidatedFile::Fasta { uri, .. }
            | UnvalidatedFile::Genbank { uri, .. }
            | UnvalidatedFile::Gfa { uri, .. }
            | UnvalidatedFile::Gff { uri, .. }
            | UnvalidatedFile::Gtf { uri, .. }
            | UnvalidatedFile::Bed { uri, .. } => uri,
        }
    }

    pub fn mut_set_path(&mut self, path: PathBuf) {
        match self {
            UnvalidatedFile::Fasta { local_path, .. }
            | UnvalidatedFile::Genbank { local_path, .. }
            | UnvalidatedFile::Gfa { local_path, .. }
            | UnvalidatedFile::Gff { local_path, .. }
            | UnvalidatedFile::Gtf { local_path, .. }
            | UnvalidatedFile::Bed { local_path, .. } => *local_path = path,
        }
    }

    #[must_use]
    pub fn set_path(self, path: PathBuf) -> Self {
        match self {
            UnvalidatedFile::Fasta { uri, .. } => UnvalidatedFile::Fasta {
                uri,
                local_path: path,
            },
            UnvalidatedFile::Genbank { uri, .. } => UnvalidatedFile::Genbank {
                uri,
                local_path: path,
            },
            UnvalidatedFile::Gfa { uri, .. } => UnvalidatedFile::Gfa {
                uri,
                local_path: path,
            },
            UnvalidatedFile::Gff { uri, .. } => UnvalidatedFile::Gff {
                uri,
                local_path: path,
            },
            UnvalidatedFile::Gtf { uri, .. } => UnvalidatedFile::Gtf {
                uri,
                local_path: path,
            },
            UnvalidatedFile::Bed { uri, .. } => UnvalidatedFile::Bed {
                uri,
                local_path: path,
            },
        }
    }

    #[must_use]
    pub fn get_path(&self) -> &Path {
        match self {
            UnvalidatedFile::Fasta { local_path, .. }
            | UnvalidatedFile::Bed { local_path, .. }
            | UnvalidatedFile::Gff { local_path, .. }
            | UnvalidatedFile::Gtf { local_path, .. }
            | UnvalidatedFile::Gfa { local_path, .. }
            | UnvalidatedFile::Genbank { local_path, .. } => local_path,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq, Clone)]
pub struct ValidatedFile {
    pub uri: String,
    // pub local_path: PathBuf,
    pub validated: bool,
    pub hash: Option<String>,
    pub last_validated: Option<Timestamp>,
}

impl Display for ValidatedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ValidatedFile {{ uri: {}, validated: {}, hash: {}, last_validated: {} }}",
            self.uri,
            self.validated,
            self.hash.as_deref().unwrap_or("None"),
            self.last_validated
                .as_ref()
                .map_or_else(|| "None".to_string(), std::string::ToString::to_string)
        )
    }
}

impl UnvalidatedFile {
    /// Attempts to validate the current `UnvalidatedFile` by verifying its contents are parseable
    /// based on the file type.
    ///
    /// This method performs validation by attempting to parse the file according to its format
    /// (FASTA, Genbank, GFA, GFF, GTF, or BED). It also generates an MD5 hash of the file
    /// contents and records the validation timestamp.
    ///
    /// # Arguments
    ///
    /// None - Uses data from the current instance
    ///
    /// # Returns
    ///
    /// Returns a `Result<ValidatedFile, ValidationError>` where:
    /// - `Ok(ValidatedFile)` - Contains the validated file metadata including URI, hash, and timestamp
    /// - `Err(ValidationError)` - Contains the specific validation error that occurred
    ///
    /// # Errors
    ///
    /// Returns a `ValidationError` if:
    /// - The file cannot be accessed due to permissions issues
    /// - The file contents are invalid/malformed for the declared format
    /// - MD5 hash calculation fails
    ///
    /// # Panics
    ///
    /// This function does not panic under normal operating conditions. All error cases are handled via
    /// the `Result` return type. However, it may panic if:
    /// - The system runs out of memory while calculating the MD5 hash
    /// - File system operations fail in an unrecoverable way (extremely rare)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let unvalidated = UnvalidatedFile::Fasta {
    ///     uri: "http://example.com/genome.fa".to_string(),
    ///     local_path: "/tmp/genome.fa".into()
    /// };
    ///
    /// match unvalidated.try_validate() {
    ///     Ok(validated) => println!("File validated successfully: {}", validated),
    ///     Err(e) => eprintln!("Validation failed: {}", e)
    /// };
    /// ```
    pub fn try_validate(&self) -> Result<ValidatedFile, ValidationError> {
        let (uri, local_path) = match self {
            UnvalidatedFile::Fasta { uri, local_path } => {
                try_parse_fasta(local_path)?;
                (uri, local_path)
            }
            UnvalidatedFile::Genbank { uri, local_path } => {
                try_parse_genbank(local_path)?;
                (uri, local_path)
            }
            UnvalidatedFile::Gfa { uri, local_path } => {
                try_parse_gfa(local_path)?;
                (uri, local_path)
            }
            UnvalidatedFile::Gff { uri, local_path } => {
                try_parse_gff(local_path)?;
                (uri, local_path)
            }
            UnvalidatedFile::Gtf { uri, local_path } => {
                try_parse_gtf(local_path)?;
                (uri, local_path)
            }
            UnvalidatedFile::Bed { uri, local_path } => {
                try_parse_bed(local_path)?;
                (uri, local_path)
            }
        };
        let hash = hash_valid_download(local_path).expect("");
        let timestamp = Timestamp::now();
        let validated = ValidatedFile {
            uri: uri.clone(),
            // local_path: local_path.clone(),
            validated: true,
            hash: Some(hash),
            last_validated: Some(timestamp),
        };

        Ok(validated)
    }

    /// Updates a [`RefDataset`] with a newly validated file, updating the appropriate file type field
    /// based on the variant of this `UnvalidatedFile`.
    ///
    /// This method performs validation on the file and updates the corresponding field in the dataset
    /// with a new [`DownloadStatus`] containing the validation results. The validation process checks
    /// both file accessibility and format correctness according to the file type.
    ///
    /// # Arguments
    ///
    /// * `self` - The `UnvalidatedFile` to validate and add to the dataset
    /// * `dataset` - Mutable reference to the [`RefDataset`] to update
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if validation and dataset update succeed, or a [`ValidationError`] if validation fails.
    ///
    /// # Errors
    ///
    /// Returns a [`ValidationError`] if:
    /// - The file cannot be accessed due to permissions or missing file
    /// - The file contents are invalid for the declared format
    /// - Parsing fails for the specific file type (FASTA, Genbank, GFA, GFF, GTF, or BED)
    ///
    /// # Example
    ///
    /// ```no_run
    /// let mut dataset = RefDataset::new();
    /// let unvalidated = UnvalidatedFile::Fasta {
    ///     uri: "path/to/file.fa".to_string(),
    ///     local_path: "local/path.fa".into()
    /// };
    /// unvalidated.update_dataset(&mut dataset)?;
    /// ```
    ///
    /// After successful execution, the dataset's corresponding field (e.g., `fasta` for a FASTA file)
    /// will contain a `DownloadStatus::Downloaded` variant with the validated file information.
    pub fn update_dataset(self, dataset: &mut RefDataset) -> Result<(), ValidationError> {
        match self {
            UnvalidatedFile::Fasta { .. } => {
                let validated = self.try_validate()?;
                let complete_download = DownloadStatus::new_downloaded(validated);
                dataset.fasta = Some(complete_download);
            }
            UnvalidatedFile::Genbank { .. } => {
                let validated = self.try_validate()?;
                let complete_download = DownloadStatus::new_downloaded(validated);
                dataset.genbank = Some(complete_download);
            }
            UnvalidatedFile::Gfa { .. } => {
                let validated = self.try_validate()?;
                let complete_download = DownloadStatus::new_downloaded(validated);
                dataset.gfa = Some(complete_download);
            }
            UnvalidatedFile::Gff { .. } => {
                let validated = self.try_validate()?;
                let complete_download = DownloadStatus::new_downloaded(validated);
                dataset.gff = Some(complete_download);
            }
            UnvalidatedFile::Gtf { .. } => {
                let validated = self.try_validate()?;
                let complete_download = DownloadStatus::new_downloaded(validated);
                dataset.gtf = Some(complete_download);
            }
            UnvalidatedFile::Bed { .. } => {
                let validated = self.try_validate()?;
                let complete_download = DownloadStatus::new_downloaded(validated);
                dataset.bed = Some(complete_download);
            }
        };

        Ok(())
    }

    /// Compares the hash of a downloaded file against a known hash to validate its integrity.
    ///
    /// This method calculates the MD5 hash of the downloaded file and compares it against a
    /// previously recorded hash value to ensure the file has not been modified or corrupted.
    /// It's particularly useful when validating that a downloaded file matches its expected
    /// contents, especially after transfers or storage.
    ///
    /// # Arguments
    ///
    /// * `self` - The `UnvalidatedFile` instance containing the file path to validate
    /// * `old_hash` - Option containing the expected MD5 hash string to compare against.
    ///   If None is provided, returns false as there is no hash to validate against.
    ///
    /// # Returns
    ///
    /// Returns a `Result<bool, ValidationError>` where:
    /// - `Ok(true)` indicates the computed hash matches the provided hash
    /// - `Ok(false)` indicates either the hashes don't match or no hash was provided
    /// - `Err(ValidationError)` indicates an error occurred during validation
    ///
    /// # Errors
    ///
    /// Returns `ValidationError::InaccessibleFile` if:
    /// - The file cannot be opened (e.g., insufficient permissions)
    /// - The file cannot be read during hash computation
    /// - The file system becomes unavailable during reading
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```rust,no_run
    /// let file = UnvalidatedFile::Fasta {
    ///     uri: "example.fa".to_string(),
    ///     local_path: "path/to/file.fa".into()
    /// };
    /// let known_hash = "d41d8cd98f00b204e9800998ecf8427e";
    /// match file.checksum(Some(known_hash)) {
    ///     Ok(true) => println!("File hash matches!"),
    ///     Ok(false) => println!("File hash mismatch or no hash provided"),
    ///     Err(e) => eprintln!("Error validating file: {}", e)
    /// }
    /// ```
    pub fn checksum(&self, old_hash: Option<&str>) -> Result<bool, ValidationError> {
        let Some(old_hash) = old_hash else {
            return Ok(false);
        };

        let downloaded_path = self.get_path();
        let new_hash = hash_valid_download(downloaded_path)?;
        let check = new_hash == old_hash;

        Ok(check)
    }
}

/// Computes the MD5 hash of a file on disk, returning it as a hexadecimal string.
///
/// This function reads the file in chunks and computes a running MD5 hash, which is useful for
/// validating file contents and detecting changes. The hash can be used to verify file integrity
/// across downloads or modifications.
///
/// # Arguments
///
/// * `download` - A path to the file to hash, can be any type that implements `AsRef<Path>`
///
/// # Returns
///
/// Returns a `Result` containing either:
/// - `Ok(String)` - The MD5 hash of the file as a lowercase hexadecimal string
/// - `Err(ValidationError)` - If the file cannot be accessed or read
///
/// # Errors
///
/// Returns `ValidationError::InaccessibleFile` if:
/// - The file cannot be opened (e.g., due to permissions or non-existence)
/// - There is an error reading the file contents
///
/// # Panics
///
/// This function does not explicitly panic, but may panic if:
/// - The system runs out of memory while reading the file
/// - The filesystem becomes unavailable during reading
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// let hash = hash_valid_download(Path::new("path/to/file.txt"))?;
/// println!("File MD5: {}", hash);
/// ```
pub fn hash_valid_download(download: impl AsRef<Path>) -> Result<String, ValidationError> {
    let Ok(file) = File::open(download.as_ref()) else {
        return Err(ValidationError::InaccessibleFile(
            "Unable to access downloaded file, indicating that file permissions may have changed."
                .to_string(),
        ));
    };
    let mut reader = BufReader::new(file);
    let mut context = Context::new();

    let mut buffer = [0u8; 64 * 1024]; // 64 KB buffer size, adjust as needed

    loop {
        let Ok(bytes_read) = reader.read(&mut buffer) else {
            return Err(ValidationError::InaccessibleFile(
                "Unable to access downloaded file, indicating that file permissions may have changed."
                    .to_string(),
            ));
        };
        if bytes_read == 0 {
            break; // EOF reached
        }
        context.consume(&buffer[..bytes_read]);
    }

    let computed: Digest = context.compute();
    let computed_hex = format!("{computed:x}");

    Ok(computed_hex)
}

/// Validates all downloaded files in a `RefDataset` to ensure they exist, are accessible, and
/// are properly formatted according to their respective file types.
///
/// This function performs parallel validation of all file types (FASTA, Genbank, GFA, GFF, GTF, BED)
/// present in the dataset. For each file type, it checks:
///
/// - If a file download is marked as completed (`DownloadStatus::Downloaded`)
/// - If the file is accessible on the filesystem
/// - If the file contents can be successfully parsed according to the expected format
///
/// The validation is done in parallel using rayon's parallel iterator to improve performance
/// when validating multiple files.
///
/// # Arguments
///
/// * `dataset` - A reference to a `RefDataset` containing the file metadata and download statuses
///   to validate
///
/// # Returns
///
/// - `Ok(())` if all files validate successfully
/// - `Err(ValidationError)` containing either a specific validation error or a collection of
///   multiple validation errors if multiple files failed validation
///
/// # Errors
///
/// This function will return a `ValidationError` if:
/// - Any file marked as downloaded cannot be accessed
/// - Any file contains malformed or invalid content for its format
/// - Multiple files fail validation (wrapped in `ValidationError::MultipleErrors`)
///
/// The specific error variants that can be returned for each file type are:
/// - FASTA: `ValidateError::InvalidFasta` or `ValidateError::InaccessibleFile`
/// - Genbank: `ValidateError::InvalidGenbank` or `ValidateError::InaccessibleFile`
/// - GFA: `ValidateError::InvalidGFA` or `ValidateError::InaccessibleFile`
/// - GFF: `ValidateError::InvalidGFF` or `ValidateError::InaccessibleFile`
/// - GTF: `ValidateError::InvalidGTF` or `ValidateError::InaccessibleFile`
/// - BED: `ValidateError::InvalidBED` or `ValidateError::InaccessibleFile`
///
/// # Examples
///
/// Basic usage:
///
/// ```no_run
/// use your_crate::RefDataset;
///
/// let dataset = RefDataset::new();
/// match validate_files(&dataset) {
///     Ok(()) => println!("All files validated successfully"),
///     Err(e) => eprintln!("Validation failed: {}", e)
/// }
/// ```
#[allow(clippy::similar_names)]
pub fn validate_files(dataset: &RefDataset) -> Result<(), ValidationError> {
    #[inline]
    fn fasta_callback(dataset_fasta: Option<&DownloadStatus>) -> Result<(), ValidationError> {
        match dataset_fasta {
            Some(status) => match status {
                DownloadStatus::NotYetDownloaded(_) => Ok(()),
                DownloadStatus::Downloaded(validated_file) => try_parse_fasta(&validated_file.uri),
            },
            None => Ok(()),
        }
    }
    #[inline]
    fn genbank_callback(dataset_genbank: Option<&DownloadStatus>) -> Result<(), ValidationError> {
        match dataset_genbank {
            Some(status) => match status {
                DownloadStatus::NotYetDownloaded(_) => Ok(()),
                DownloadStatus::Downloaded(validated_file) => {
                    try_parse_genbank(&validated_file.uri)
                }
            },
            None => Ok(()),
        }
    }
    #[inline]
    fn gfa_callback(dataset_gfa: Option<&DownloadStatus>) -> Result<(), ValidationError> {
        match dataset_gfa {
            Some(status) => match status {
                DownloadStatus::NotYetDownloaded(_) => Ok(()),
                DownloadStatus::Downloaded(validated_file) => try_parse_gfa(&validated_file.uri),
            },
            None => Ok(()),
        }
    }
    #[inline]
    fn gff_callback(dataset_gff: Option<&DownloadStatus>) -> Result<(), ValidationError> {
        match dataset_gff {
            Some(status) => match status {
                DownloadStatus::NotYetDownloaded(_) => Ok(()),
                DownloadStatus::Downloaded(validated_file) => try_parse_gff(&validated_file.uri),
            },
            None => Ok(()),
        }
    }
    #[inline]
    fn gtf_callback(dataset_gtf: Option<&DownloadStatus>) -> Result<(), ValidationError> {
        match dataset_gtf {
            Some(status) => match status {
                DownloadStatus::NotYetDownloaded(_) => Ok(()),
                DownloadStatus::Downloaded(validated_file) => try_parse_gtf(&validated_file.uri),
            },
            None => Ok(()),
        }
    }
    #[inline]
    fn bed_callback(dataset_bed: Option<&DownloadStatus>) -> Result<(), ValidationError> {
        match dataset_bed {
            Some(status) => match status {
                DownloadStatus::NotYetDownloaded(_) => Ok(()),
                DownloadStatus::Downloaded(validated_file) => try_parse_bed(&validated_file.uri),
            },
            None => Ok(()),
        }
    }
    let validation_callbacks = vec![
        fasta_callback(dataset.fasta.as_ref()),
        genbank_callback(dataset.genbank.as_ref()),
        gfa_callback(dataset.gfa.as_ref()),
        gff_callback(dataset.gff.as_ref()),
        gtf_callback(dataset.gtf.as_ref()),
        bed_callback(dataset.bed.as_ref()),
    ]
    .into_par_iter()
    .filter_map(std::result::Result::err)
    .collect::<Vec<ValidationError>>();

    if !validation_callbacks.is_empty() {
        return Err(ValidationError::MultipleErrors(
            crate::MultipleValidationErrors(validation_callbacks),
        ));
    }

    Ok(())
}

fn try_parse_fasta(file: impl AsRef<Path>) -> Result<(), ValidationError> {
    if file.as_ref().ends_with(".fasta") {
        let Ok(mut fa_reader) = File::open(file.as_ref())
            .map(BufReader::new)
            .map(fasta::Reader::new)
        else {
            return Err(ValidationError::InaccessibleFile(
                file.as_ref().to_string_lossy().into_owned(),
            ));
        };
        while let Some(record) = fa_reader.records().next() {
            match record {
                Ok(_) => continue,
                Err(msg) => return Err(ValidationError::InvalidFasta(format!("{msg}"))),
            }
        }
    } else if file.as_ref().extension().is_some_and(|ext| ext == "gz") {
        let Ok(mut fa_reader) = File::open(file.as_ref())
            .map(BufReader::new)
            .map(GzDecoder::new)
            .map(BufReader::new)
            .map(fasta::Reader::new)
        else {
            return Err(ValidationError::InaccessibleFile(
                file.as_ref().to_string_lossy().into_owned(),
            ));
        };
        while let Some(record) = fa_reader.records().next() {
            match record {
                Ok(_) => continue,
                Err(msg) => return Err(ValidationError::InvalidFasta(format!("{msg}"))),
            }
        }
    }
    Ok(())
}

fn try_parse_genbank(file: impl AsRef<Path>) -> Result<(), ValidationError> {
    let Ok(gbk_reader) = File::open(file.as_ref())
        .map(BufReader::new)
        .map(gb_io::reader::SeqReader::new)
    else {
        return Err(ValidationError::InaccessibleFile(
            file.as_ref().to_string_lossy().into_owned(),
        ));
    };

    for record in gbk_reader {
        match record {
            Ok(_) => continue,
            Err(msg) => return Err(ValidationError::InvalidGenbank(format!("{msg}"))),
        }
    }

    Ok(())
}

fn try_parse_gfa(file: impl AsRef<Path>) -> Result<(), ValidationError> {
    let Ok(_) = gfa::parser::GFAParserBuilder::all()
        .pedantic_errors()
        .segments(false)
        .build_bstr_id::<()>()
        .parse_file(&file)
    else {
        return Err(ValidationError::InvalidGFA(
            file.as_ref().to_string_lossy().into_owned(),
        ));
    };

    Ok(())
}

fn try_parse_gff(file: impl AsRef<Path>) -> Result<(), ValidationError> {
    let Ok(mut gff_reader) = File::open(file.as_ref())
        .map(BufReader::new)
        .map(gff::Reader::new)
    else {
        return Err(ValidationError::InaccessibleFile(
            file.as_ref().to_string_lossy().into_owned(),
        ));
    };
    while let Some(record) = gff_reader.record_bufs().next() {
        match record {
            Ok(_) => continue,
            Err(msg) => return Err(ValidationError::InvalidGFF(format!("{msg}"))),
        }
    }
    Ok(())
}

fn try_parse_gtf(file: impl AsRef<Path>) -> Result<(), ValidationError> {
    let Ok(mut gff_reader) = File::open(file.as_ref())
        .map(BufReader::new)
        .map(gtf::Reader::new)
    else {
        return Err(ValidationError::InaccessibleFile(
            file.as_ref().to_string_lossy().into_owned(),
        ));
    };
    while let Some(record) = gff_reader.record_bufs().next() {
        match record {
            Ok(_) => continue,
            Err(msg) => return Err(ValidationError::InvalidGTF(format!("{msg}"))),
        }
    }
    Ok(())
}

fn try_parse_bed(file: impl AsRef<Path>) -> Result<(), ValidationError> {
    let Ok(mut bed_reader) = File::open(file.as_ref())
        .map(BufReader::new)
        .map(bed::Reader::<3, _>::new)
    else {
        return Err(ValidationError::InaccessibleFile(
            file.as_ref().to_string_lossy().into_owned(),
        ));
    };
    let mut record = bed::Record::default();
    match bed_reader.read_record(&mut record) {
        Ok(_) => Ok(()),
        Err(msg) => Err(ValidationError::InvalidBED(format!("{msg}"))),
    }
}
