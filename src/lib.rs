#![warn(clippy::pedantic, clippy::perf)]

pub mod cli;
pub mod data;
pub mod downloads;
pub mod prelude;
pub mod project;
pub mod validate;

// re-exports
pub use prelude::*;
