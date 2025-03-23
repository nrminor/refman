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
}
