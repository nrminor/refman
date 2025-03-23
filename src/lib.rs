#![warn(clippy::pedantic, clippy::perf)]

pub mod cli;
pub mod data;
pub mod downloads;
pub mod prelude;
pub mod project;

// re-exports
pub use prelude::*;
