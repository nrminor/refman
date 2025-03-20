# `RefMan`: a simple biological reference manager

Bioinformatics involves juggling lots of files, particularly reference datasets (FASTA, GenBank, EMBL, Oh My!) with associated annotation and genomic range data. `refman` evolved out of [our internal](https://dho.pathology.wisc.edu/) desire to simplify accessing references from many sources--both official and bespoke--as well as my own desire to write more Rust🦀.

`refman` can be thought of as a simpler and less general-purpose implementation of what [SciDataFlow](https://github.com/vsbuffalo/scidataflow) does. For uses cases beyond getting a few reference datasets from disparate places, I highly recommend giving SciDataFlow a try. But if you're like me and your head is spinning with all the different combinations of all the reference datasets each of your projects needs, and you want a fast way of pulling these combinations together, `refman` is for you!

## Installation

### Precompiled Binary Releases

Precompiled static binaries for a variety of platforms are available in [`refman`'s Github release](https://github.com/nrminor/refman/releases).

### Crates.io

Since v1.0.0, `refman` has been available on [crates.io](https://crates.io/crates/refman). Assuming you have the [Rust toolchain](https://www.rust-lang.org/tools/install) installed, simply install it with `cargo install refman`.

### Build from source

If you have git available, are on a unix system, and have the [Rust toolchain](https://www.rust-lang.org/tools/install) installed, you can download and compile `refman` from source with the following:

```bash
# download the source code with git clone
git clone https://github.com/nrminor/refman

# change into the project root directory
cd refman

# compile and install it onto your $PATH with cargo
cargo install --path="."
```

## Quick Start

`refman` centers around a workflow of three subcommands: `refman init`, `refman register`, and `refman download`. These commands initialize a project with metadata, register dataset URLs, and download datasets respectively. Use `--help` or `-h` on each subcommand to explore the command line interface. The top-level interface will look like this when you run `refman -h`:

```

░       ░░░        ░░        ░░  ░░░░  ░░░      ░░░   ░░░  ░
▒  ▒▒▒▒  ▒▒  ▒▒▒▒▒▒▒▒  ▒▒▒▒▒▒▒▒   ▒▒   ▒▒  ▒▒▒▒  ▒▒    ▒▒  ▒
▓       ▓▓▓      ▓▓▓▓      ▓▓▓▓        ▓▓  ▓▓▓▓  ▓▓  ▓  ▓  ▓
█  ███  ███  ████████  ████████  █  █  ██        ██  ██    █
█  ████  ██        ██  ████████  ████  ██  ████  ██  ███   █

refman (v1.0.0)
------------------------------------------------------------
`refman` is a simple command-line tool for managing biological reference datasets often
used in bioinformatics. These datasets may include raw sequence files, files encoding
annotations on those sequences, etc. `refman` makes it easier to manage and download
these kinds of files globally on the user's machine, or on a per-project basis. It
uses a human-readable TOML file to track which files it's managing, which can be shared
between users to aid scientific reproducibility.


Usage: refman [OPTIONS] [COMMAND]

Commands:
  init      Initialize a registry for the current project without registering any datasets. [aliases: i, new]
  register  Register a new file or set of files with a given dataset label. [aliases: r, reg]
  remove    Remove the files associated with a given dataset label [aliases: rm]
  list      List all previously registered reference datasets [aliases: l]
  download  Download one or many reference datasets registered in the refman registry. [aliases: d, dl, down, get, fetch]
  help      Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose...  Increase logging verbosity
  -q, --quiet...    Decrease logging verbosity
  -h, --help        Print help (see more with '--help')
  -V, --version     Print version

```

## Detailed Usage

`refman`'s first deployment was in the bioinformatic pipeline [`oneroof`](https://github.com/nrminor/oneroof), which is run routinely with different reference datasets depending on the input data. These datasets were registered in the pipeline's [`refman.toml`](https://github.com/nrminor/oneroof/blob/main/refman.toml) file with the same series of commands that would be used with any project. To demonstrate this workflow, those commands are reproduced here.

#### Project Initialization

First, to use `refman` as part of `oneroof`'s setup, we initialize a project, like so:

```bash
refman init -t oneroof -d "Reference files needed for routine runs on viral amplicon data from SARS-CoV-2 and H5N1"
```

This will create a `refman.toml` that looks like this:

```toml
[project]
title = "oneroof"
description = "Reference files needed for routine runs on viral amplicon data from SARS-CoV-2 and H5N1"
last_modified = "2025-03-19T17:24:04.673076Z"
global = false
datasets = []
```

No datasets have been registered yet. This `refman.toml` also uses the tool's default behavior, which includes making each `refman.toml` local to a project instead of global to a machine. Support for global usage will increase with time, but in general we recommend that `refman` is used on a per-project basis to avoid confusion.

#### Registering URLs with Datasets

Next, datasets for a few `oneroof` configurations were registered, like so:

```bash
# first, datasets for running oneroof on SARS-CoV-2 amplicons with the QIASeq Direct Enhanced Boosted primer set
refman register -l "sc2-qiaseq" \
--fasta "https://dholk.primate.wisc.edu/_webdav/dho/public/DHO%20Lab%20Bespoke%20Reference%20Dataset%20Registry/Pathogen%20Genomics/%40files/sars-cov-2/MN908947.3.fasta" \
--genbank "https://dholk.primate.wisc.edu/_webdav/dho/public/DHO%20Lab%20Bespoke%20Reference%20Dataset%20Registry/Pathogen%20Genomics/%40files/sars-cov-2/MN908947.3.gbk" \
--gff "https://dholk.primate.wisc.edu/_webdav/dho/public/DHO%20Lab%20Bespoke%20Reference%20Dataset%20Registry/Pathogen%20Genomics/%40files/sars-cov-2/MN908947.3_corrected_orf1.gff" \
--bed "https://dholk.primate.wisc.edu/_webdav/dho/public/DHO%20Lab%20Bespoke%20Reference%20Dataset%20Registry/Pathogen%20Genomics/%40files/sars-cov-2/qiaseq_direct_boosted.bed"

# second, datasets for H5N1 amplicons from our own bespoke H5N1 tiled primer set
refman register -l "h5n1-B-custom" \
--fasta "https://dholk.primate.wisc.edu/_webdav/dho/public/DHO%20Lab%20Bespoke%20Reference%20Dataset%20Registry/Pathogen%20Genomics/%40files/H5N1-B.3.13/custom_reference.fasta?contentDisposition=attachment" \
--genbank "https://dholk.primate.wisc.edu/_webdav/dho/public/DHO%20Lab%20Bespoke%20Reference%20Dataset%20Registry/Pathogen%20Genomics/%40files/H5N1-B.3.13/annotation-custom.gbk?contentDisposition=attachment" \
--bed "https://dholk.primate.wisc.edu/_webdav/dho/public/DHO%20Lab%20Bespoke%20Reference%20Dataset%20Registry/Pathogen%20Genomics/%40files/H5N1-B.3.13/final_truth_no_dashes.bed?contentDisposition=attachment"

# and third, a simpler dataset for H5N1 whole-segment amplicons for sequencing on Oxford Nanopore instruments
refman register -l "h5n1-B-segmental" \
--fasta "https://dholk.primate.wisc.edu/_webdav/dho/public/DHO%20Lab%20Bespoke%20Reference%20Dataset%20Registry/Pathogen%20Genomics/%40files/H5N1-B.3.13/h5_cattle_genome_root_segments.fasta?contentDisposition=attachment"
```

Note that, as documented in the help menu for `refman`, `reg` and `r` are aliases for the `register` subcommand. All `refman` subcommands have shorthand aliases. Also, URLs _must be provided between quotes_.

Before completing the registration process, `refman` uses the Rust [`lychee` library](https://crates.io/crates/lychee-lib) to check that each provided URL is valid and points to resource that exists. This prevents invalid entries to `refman.toml` when managed through the command-line interface.

#### Downloading Datasets 

Once these datasets are registered, they can later be deserialized from `refman.toml` and used to download those resources. For `oneroof`, this most often involves downloading datasets for a SARS-CoV-2 run, like so:

```bash
refman download sc2-qiaseq -d assets
```

This will download all the files in the dataset labeled "sc2-qiaseq" (registered above) and place them in a destination directory called "assets". Like in the `register` subcommand, URLs will be checked for validity before being used to download files. Note that the dataset label used is case-sensitive and must exactly match a dataset registered with `refman`.

Keep in mind that if you're coming to a new project with datasets managed with `refman`, you can always list what's available with `refman list`, and list full URLs for particular projects with `refman list <LABEL>`.

## Roadmap

`refman` reached v1.0.0 as a minimum viable product, but it's still a work in progress. Features on the roadmap include:

- [ ] more black-box-, white-box-, and especially doc-tests
- [x] link-checking with [lychee_lib](https://docs.rs/lychee-lib/latest/lychee_lib/)
- [x] download progress bars
- [ ] a GitHub workflow for generating releases so that static binaries are available
- [x] publication on [crates.io](https://crates.io/)
- [ ] the ability to symlink locations in the local filesystem as opposed to only pulling from the internet
- [ ] expanded metadata fields or file formats, e.g., VCFs
- [ ] validation that an entry in a given file format is actually that format
- [ ] tasks/rules that tell `refman` to do some operation on a file once it's downloaded, potentially in an embedded scripting language like [Lua](https://www.lua.org/) or [Gluon](https://github.com/gluon-lang/gluon)
- [x] stable config file format
- [ ] a global dotfile format with higher precedence than the tool's current defaults
- [ ] a python API with a restricted feature set that is pip-installable

If you're interested in speeding any of these or other features along, or find any bugs, please reach out in [the repo's issues](https://github.com/nrminor/refman/issues)!

#### Non-goals

1. Maximal performance. The `refman` code contains a few clones here and there where it's convenient, though never for large amounts of data.
2. Minimal dependencies. I've used this project in part to explore some interesting [crates](https://crates.io/) from the Rust ecosystem that I haven't used previously.

## Citation(s)

Coming soon.

