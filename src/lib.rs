#![warn(clippy::pedantic, clippy::perf, clippy::todo, clippy::expect_used)]
// #![forbid(clippy::unwrap_used)]

pub mod cli;
pub mod data;
pub mod downloads;
pub mod errors;
pub mod prelude;
pub mod project;
pub mod validate;

// re-exports
pub use prelude::*;
