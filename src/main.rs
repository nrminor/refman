#![warn(
    clippy::pedantic,
    clippy::perf,
    clippy::todo,
    clippy::expect_used,
    clippy::unwrap_used
)]

use std::path::PathBuf;

use clap::Parser;
use clap_verbosity_flag::Verbosity;
use color_eyre::{eyre::Context, Result};
use fern::colors::{Color, ColoredLevelConfig};
use refman::{
    cli::{self, Cli, Commands},
    prelude::*,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Parse provided command line arguments
    let cli = Cli::parse();

    // Determine how much verbosity the user requested and use that level to set up logging
    let verbosity = cli.verbose;
    setup_logger(verbosity)?;

    // Run the called subcommand or print info
    match cli.command {
        // if no subcommand is provided in the command-line, just print the tool's info.
        None => {
            eprintln!("{}\n", cli::INFO);
            std::process::exit(0);
        }

        // simple command that creates a new file that can be used to track reference datasets
        Some(Commands::Init {
            registry,
            global,
            title,
            description,
        }) => {
            let options = RegistryOptions::try_new(title, description, &registry, global)?;
            options.init()?;
            Ok(())
        }

        // The register subcommand adds a new entry to refman.toml that includes at least one valid URL
        // along with a dataset label
        Some(Commands::Register {
            label,
            fasta,
            genbank,
            gfa,
            gtf,
            gff,
            bed,
            tar,
            registry,
            global,
        }) => {
            let new_dataset =
                RefDataset::try_new(label, fasta, genbank, gfa, gff, gtf, bed, tar).await?;
            let options = RegistryOptions::try_new(None, None, &registry, global)?;
            let mut project = options.read_registry()?.register(new_dataset).await?;
            options.write_registry(&mut project)?;
            Ok(())
        }

        // The remove subcommand removes a dataset that was previously registered with refman
        Some(Commands::Remove {
            label,
            registry,
            global,
        }) => {
            let options = RegistryOptions::try_new(None, None, &registry, global)?;
            let mut project = options.read_registry()?.remove(&label)?;
            options.write_registry(&mut project)?;
            Ok(())
        }

        // The list subcommand prints the registered datasets in a human-readable table
        Some(Commands::List {
            registry,
            global,
            label,
        }) => {
            RegistryOptions::try_new(None, None, &registry, global)?
                .read_registry()?
                .prettyprint(label);
            Ok(())
        }

        // the download subcommand pulls the data from a previously registered dataset
        Some(Commands::Download {
            label,
            registry,
            dest,
            global,
            all,
        }) => {
            // setup up registry options if provided
            let options = RegistryOptions::try_new(None, None, &registry, global)?;

            // set up the destination path
            let destination = dest.unwrap_or_else(|| PathBuf::from("."));

            // read in the project data
            let project = options.read_registry()?;

            let Some(ref provided_label_str) = label else {
                let mut updated_project = project.download_dataset(None, destination).await?;
                options.write_registry(&mut updated_project)?;
                return Ok(());
            };

            if all {
                let mut updated_project = project
                    .download_dataset(label.as_deref(), destination)
                    .await?;
                options.write_registry(&mut updated_project)?;
                return Ok(());
            }

            if !project.is_registered(provided_label_str) {
                Err(RegistryError::NotRegistered(provided_label_str.to_string()))?;
            }

            let mut updated_project = project
                .download_dataset(label.as_deref(), destination)
                .await?;
            options.write_registry(&mut updated_project)?;

            Ok(())
        }
    }
}

fn setup_logger(verbosity: Verbosity) -> Result<()> {
    // set up the logging verbosity as provided by the user
    let level = verbosity.log_level_filter();

    // set colors for the logs based on their level, because why not
    let colors = ColoredLevelConfig::new()
        .trace(Color::BrightBlue)
        .debug(Color::Blue)
        .warn(Color::Yellow)
        .error(Color::Red)
        .info(Color::Green);

    // build and apply a new logger instance user fern and the user's desired verbosity
    fern::Dispatch::new()
        .level(level)
        .level_for("hyper", log::LevelFilter::Warn)
        .level_for("clap", log::LevelFilter::Warn)
        .level_for("clap_builder", log::LevelFilter::Warn)
        .format(move |out, message, record| {
            out.finish(format_args!(
                "[{} {} {}] {}",
                jiff::Timestamp::now(),
                colors.color(record.level()),
                record.target(),
                message,
            ));
        })
        .chain(std::io::stderr())
        .apply()
        .with_context(|| "Failed to setup logging.")?;

    Ok(())
}
