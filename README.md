# `RefMan`: a simple biological reference manager

Bioinformatics involves juggling lots of files, particularly reference datasets (FASTA, GenBank, EMBL, Oh My!) with associated annotation and genomic range data. `refman` evolved out of [our internal](https://dho.pathology.wisc.edu/) desire to simplify accessing references from many sources--both official and bespoke--as well as my own desire to write more Rust🦀.

`refman` can be thought of as a simpler and less general-purpose implementation of what [SciDataFlow](https://github.com/vsbuffalo/scidataflow) does. For uses cases beyond getting a few reference datasets from disparate places, I highly recommend giving SciDataFlow a try. But if you're like me and your head is spinning with all the different combinations of all the reference datasets each of your projects needs, and you want a fast way of pulling these combinations together, `refman` is for you!

## Roadmap

`refman` is still a work in progress. Still on the roadmap are:

- [ ] more black-box-, white-box-, and especially doc-tests
- [ ] link-checking with [lychee_lib](https://docs.rs/lychee-lib/latest/lychee_lib/)
- [ ] download progress bars
- [ ] a GitHub workflow for generating releases so that static binaries are available
- [ ] publication on [crates.io](https://crates.io/)
- [ ] potential API access to well-known repositories or other data stores
- [ ] the ability to symlink locations in the local filesystem as opposed to only pulling from the internet
- [ ] expanded metadata fields or file formats, e.g., VCFs
- [ ] validation that an entry in a given file format is actually that format
- [ ] tasks/rules that tell `refman` to do some operation on a file once it's downloaded, potentially in an embedded scripting language like [Lua](https://www.lua.org/) or [Gluon](https://github.com/gluon-lang/gluon)
- [ ] stable config file format
- [ ] a global dotfile format with higher precedence than the tool's current defaults
- [ ] a python API with a restricted feature set that is pip-installable

If you're interested in speeding any of these or other features along, or find any bugs, please reach out in [the repo's issues](https://github.com/nrminor/refman/issues)!

#### Non-goals

1. Maximal performance. The `refman` code contains a few clones here and there where it's convenient, though never for large amounts of data.
2. Minimal dependencies. I've used this project in part to explore some interesting [crates](https://crates.io/) from the Rust ecosystem that I haven't used previously.

## Installation

More coming soon. For now, assuming you have git available, are on a unix system, and have the [Rust toolchain](https://www.rust-lang.org/tools/install) installed, you can download and compile it from source with the following:

```bash
# download the source code with git clone
git clone https://github.com/nrminor/refman

# change into the project root directory
cd refman

# compile and install it onto your $PATH with cargo
cargo install --path="."
```

## Quick Start

Coming soon.

## Usage

Coming soon.

## Citation(s)

Coming soon.

