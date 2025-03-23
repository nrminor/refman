use std::path::PathBuf;

use clap::{Parser, Subcommand};

pub const INFO: &str = r"

░       ░░░        ░░        ░░  ░░░░  ░░░      ░░░   ░░░  ░
▒  ▒▒▒▒  ▒▒  ▒▒▒▒▒▒▒▒  ▒▒▒▒▒▒▒▒   ▒▒   ▒▒  ▒▒▒▒  ▒▒    ▒▒  ▒
▓       ▓▓▓      ▓▓▓▓      ▓▓▓▓        ▓▓  ▓▓▓▓  ▓▓  ▓  ▓  ▓
█  ███  ███  ████████  ████████  █  █  ██        ██  ██    █
█  ████  ██        ██  ████████  ████  ██  ████  ██  ███   █

refman (v0.1.0)
------------------------------------------------------------
`refman` is a simple command-line tool for managing biological reference datasets often
used in bioinformatics. These datasets may include raw sequence files, files encoding
annotations on those sequences, etc. `refman` makes it easier to manage and download
these kinds of files globally on the user's machine, or on a per-project basis. It
uses a human-readable TOML file to track which files it's managing, which can be shared
between users to aid scientific reproducibility.
";

#[derive(Parser)]
#[clap(name = "refman")]
#[clap(about = INFO)]
#[clap(version = "v1.1.1")]
pub struct Cli {
    #[command(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// This enum encodes the CLI subcommands that `refman` exposes to users. Each variant
/// represents a different operation that can be performed:
///
/// - `Init`: Creates a new reference registry without registering any datasets yet
/// - `Register`: Add a new dataset entry to the registry with an associated label
/// - `Remove`: Delete an existing dataset from the registry by its label
/// - `List`: Show all datasets currently in the registry
/// - `Download`: Fetch registered dataset files to the local filesystem
///
/// Each command takes various arguments to customize its behavior, like whether to use
/// a global vs project-local registry, custom file paths, etc. Most commands require
/// at minimum a dataset label to identify which reference data to operate on.
///
/// The registry itself is stored as a TOML file that tracks metadata about each
/// registered dataset, including download URLs for supported file formats like FASTA,
/// Genbank, GFF, etc.
#[derive(Subcommand)]
pub enum Commands {
    #[clap(
        about = "Initialize a registry for the current project without registering any datasets.",
        visible_aliases = &["i", "new"],
    )]
    Init {
        /// Optional project title
        #[arg(short, long, required = false)]
        title: Option<String>,

        /// Optional project description
        #[arg(short, long, required = false)]
        description: Option<String>,

        /// Optional file path (absolute or relative) to the refget registry file.
        #[arg(short, long, required = false)]
        registry: Option<String>,

        /// Whether to use a global registry as opposed to a project-specific registry
        #[arg(short, long, required = false)]
        global: bool,
    },

    #[clap(
        about = "Register a new file or set of files with a given dataset label.",
        visible_aliases = &["r", "reg"],
    )]
    Register {
        /// Shorthand label for a dataset to register with refman. Once registered, this shorthand can be used
        /// to download and manage reference datasets in the future.
        #[arg(index = 1, required = true)]
        label: String,

        /// URL to simple reference sequence in FASTA format
        #[arg(long, required = false)]
        fasta: Option<String>,

        /// URL to reference sequence, potentially with annotations, in Genbank format
        #[arg(long, required = false)]
        genbank: Option<String>,

        /// URL to reference assemly in Graphical Fragment Assembly (GFA) format
        #[arg(long, required = false)]
        gfa: Option<String>,

        /// URL to reference annotation data in Gene Transfer Format (GTF)
        #[arg(long, required = false)]
        gtf: Option<String>,

        /// URL to reference annotation data in General Feature Format (GFF3)
        #[arg(long, required = false)]
        gff: Option<String>,

        /// URL to reference range data in Browser Extensible Data (BED) format
        #[arg(long, required = false)]
        bed: Option<String>,

        /// Optional file path (absolute or relative) to the refget registry file.
        #[arg(short, long, required = false)]
        registry: Option<String>,

        /// Whether to use a global registry as opposed to a project-specific registry
        #[arg(short, long, required = false)]
        global: bool,
    },

    #[clap(
        about = "Remove the files associated with a given dataset label",
        visible_aliases = &["rm"],
    )]
    Remove {
        /// Shorthand label for a dataset to register with refman. Once registered, this shorthand can be used
        /// to download and manage reference datasets in the future.
        #[arg(index = 1, required = true)]
        label: String,

        /// Optional file path (absolute or relative) to the refget registry file.
        #[arg(short, long, required = false)]
        registry: Option<String>,

        /// Whether to use a global registry as opposed to a project-specific registry
        #[arg(short, long, required = false)]
        global: bool,
    },

    #[clap(
        about = "List all previously registered reference datasets",
        visible_aliases = &["l"],
    )]
    List {
        /// Label string for a registered dataset
        #[arg(index = 1, required = false)]
        label: Option<String>,

        /// Optional file path (absolute or relative) to the refget registry file.
        #[arg(short, long, required = false)]
        registry: Option<String>,

        /// Whether to use a global registry as opposed to a project-specific registry
        #[arg(short, long, required = false)]
        global: bool,
    },

    #[clap(
        about = "Download one or many reference datasets registered in the refman registry.",
        visible_aliases = &["d", "dl", "down", "get", "fetch"]
    )]
    Download {
        /// Label string for a registered dataset
        #[arg(index = 1, required = true)]
        label: String,

        /// Destination directory for downloaded files, defaulting to the current working directory.
        #[arg(short, long, required = false)]
        dest: Option<PathBuf>,

        /// Optional file path (absolute or relative) to the refget registry file.
        #[arg(short, long, required = false)]
        registry: Option<String>,

        /// Whether to use a global registry as opposed to a project-specific registry
        #[arg(short, long, required = false)]
        global: bool,
    },
}
