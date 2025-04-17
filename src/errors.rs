use std::{error, fmt, io};
use thiserror::Error;
use toml::{de, ser};

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error(
        "The file provided for validation, `{0}`, is inaccessible, either because of insufficient read permissions or because it does not exist."
    )]
    InaccessibleFile(String),
    #[error(
        "The file provided as FASTA format, `{0}`, could not be parsed and validated in that format, and thus will not be registered."
    )]
    InvalidFasta(String),
    #[error(
        "The file provided as Genbank format, `{0}`, could not be parsed and validated in that format, and thus will not be registered."
    )]
    InvalidGenbank(String),
    #[error(
        "The file provided as GFA format, `{0}`, could not be parsed and validated in that format, and thus will not be registered."
    )]
    InvalidGFA(String),
    #[error(
        "The file provided as GFF format, `{0}`, could not be parsed and validated in that format, and thus will not be registered."
    )]
    InvalidGFF(String),
    #[error(
        "The file provided as GTF format, `{0}`, could not be parsed and validated in that format, and thus will not be registered."
    )]
    InvalidGTF(String),
    #[error(
        "The file provided as BED format, `{0}`, could not be parsed and validated in that format, and thus will not be registered. Note that BED files must at least have three columns: the reference contig ID in a corresponding FASTA file, the start coordinate, and the stop coordinate. Additional fields may be included according to the BED specification, but they are not validated here."
    )]
    InvalidBED(String),
    #[error("Multiple validation errors occurred:\n{0}")]
    MultipleErrors(MultipleValidationErrors),
}

#[derive(Debug)]
pub struct MultipleValidationErrors(pub Vec<ValidationError>);
impl fmt::Display for MultipleValidationErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for error in &self.0 {
            writeln!(f, "- {error}")?;
        }
        Ok(())
    }
}

impl error::Error for MultipleValidationErrors {}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error(
        "A dataset register or download was requested, but a refman registry does not yet exist. Please initialize it with `refman init`, or initialize it implicitly by adding your first dataset with `refman register`."
    )]
    NoRegistry,
    #[error(
        "A registry file was found, but the file is empty, likely because `refman init` was run without registering anything. Run `refman register` to start filling the file."
    )]
    EmptyRegistry,
    #[error(
        "The requested file `{0}` has not been registered yet. To download it, please register it with a label with `refman register`."
    )]
    NotRegistered(String),
    #[error(
        "An invalid or inacccessible directory for storing the refman registry was provided. Make sure that the current or requested directory still exists and that the current user has write permissions there."
    )]
    InvalidPath(#[from] io::Error),
    #[error(
        "TOML registry format was invalid and could not be deserialized. A new registry may need to be initialized."
    )]
    InvalidInputFormat(#[from] de::Error),
    #[error(
        "The internal project representation was invalid, and thus cannot be serialized into the the TOML registry format."
    )]
    InvalidOutputFormat(#[from] ser::Error),
    #[error("unknown refman error")]
    Unknown,
}

#[derive(Debug, Error)]
pub enum EntryError {
    #[error(
        "A label for a reference dataset was provided without any files. Please include at least one file per label."
    )]
    LabelButNoFiles,
    #[error(
        "Annotations for `{0}` were registered or requested without an associated sequence in FASTA or Genbank format."
    )]
    AnnotationsButNoSequence(String),
    #[error("The provided label `{0}` is not present in the refman registry.")]
    LabelNotFound(String),
    #[error(
        "The label `{0}` is the final entry in the refman registry, which will leave behind an invalid state. Please delete the `refman.toml` file to proceed."
    )]
    FinalEntry(String),
    #[error(
        "The URL provided to be registered is invalid or does not point to a resource that exists."
    )]
    InvalidURL(#[from] color_eyre::Report),
}

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("")]
    InvalidUrl,
    #[error("")]
    NetworkError,
}
