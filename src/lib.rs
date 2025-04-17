// crate-level lints
#![warn(
    clippy::pedantic,
    clippy::perf,
    clippy::todo,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::complexity,
    clippy::correctness,
    clippy::absolute_paths,
    clippy::style
)]

// public modules
pub mod cli;
pub mod data;
pub mod prelude;
pub mod project;

// private internals
mod downloads;
mod errors;
mod global;
mod validate;

// re-exports
pub use prelude::*;
