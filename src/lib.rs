#![allow(clippy::pedantic, clippy::perf)]

pub mod cli;
pub mod data;
pub mod downloads;
pub mod project;

pub mod prelude {

    // re-exports
    pub use crate::data::RefDataset;
    pub use crate::project::{Project, RegistryOptions};

    use std::io;

    use thiserror::Error;

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
        InvalidInputFormat(#[from] toml::de::Error),
        #[error(
            "The internal project representation was invalid, and thus cannot be serialized into the the TOML registry format."
        )]
        InvalidOutputFormat(#[from] toml::ser::Error),
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
        InvalidURL(#[from] anyhow::Error),
    }

    #[derive(Debug, Error)]
    pub enum DownloadError {
        #[error("")]
        InvalidUrl,
        #[error("")]
        NetworkError,
    }
}
pub use prelude::*;
