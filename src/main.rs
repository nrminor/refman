#![warn(clippy::pedantic, clippy::perf)]

use std::env;

use anyhow::{Context, Result};
use clap::Parser;
use clap_verbosity_flag::Verbosity;
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
            registry,
            global,
        }) => {
            let new_dataset =
                RefDataset::try_new(label, fasta, genbank, gfa, gff, gtf, bed).await?;
            let options = RegistryOptions::try_new(None, None, &registry, global)?;
            let mut project = options.read_registry()?.register(new_dataset)?;
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
        }) => {
            let options = RegistryOptions::try_new(None, None, &registry, global)?;
            let project = options.read_registry()?;
            if !project.is_registered(&label) {
                Err(RegistryError::NotRegistered(label.clone()))?;
            }
            let destination = match dest {
                Some(dest) => dest,
                None => env::current_dir()?,
            };

            project.download_dataset(&label, destination).await?;

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
