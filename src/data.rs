use serde::{Deserialize, Serialize};

use crate::{EntryError, downloads::check_url};

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
#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
pub struct RefDataset {
    pub label: String,
    // TODO: Some ideas on fields to add
    // hash: &[u8],
    // db_source: String,
    // db_accession: String,
    pub fasta: Option<String>,
    pub genbank: Option<String>,
    pub gfa: Option<String>,
    pub gff: Option<String>,
    pub gtf: Option<String>,
    pub bed: Option<String>,
}

impl RefDataset {
    /// Fully public new method that attempts to initialize a reference dataset entry while enforcing a few invariants,
    /// including that an annotation file can only ever be registered if it comes with a sequence to pull from, and
    /// that a label cannot be registered without at least one file.
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
            | (None, None, None, Some(label), None)
            | (None, None, None, Some(label), Some(_))
            | (None, None, Some(label), None, None)
            | (None, None, Some(label), None, Some(_))
            | (None, None, Some(label), Some(_), None)
            | (None, None, Some(label), Some(_), Some(_)) => {
                Err(EntryError::AnnotationsButNoSequence(label.to_string()))
            }

            // If none of the above conditions are met, we're all good! Return an instance of the `RefDataset` struct
            // with validated combinations of fields.
            _ => {
                // check each of the possible files, if provided by the user
                if let Some(url_to_check) = &fasta {
                    let _ = check_url(url_to_check).await?;
                }
                if let Some(url_to_check) = &genbank {
                    let _ = check_url(url_to_check).await?;
                }
                if let Some(url_to_check) = &gfa {
                    let _ = check_url(url_to_check).await?;
                }
                if let Some(url_to_check) = &gff {
                    let _ = check_url(url_to_check).await?;
                }
                if let Some(url_to_check) = &gtf {
                    let _ = check_url(url_to_check).await?;
                }
                if let Some(url_to_check) = &bed {
                    let _ = check_url(url_to_check).await?;
                }

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

    pub fn label(&self) -> &str {
        &self.label
    }
}
