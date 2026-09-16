//! Dependency-graph orchestration for Scoop builds.
//!
//! This crate may inspect manifests and `.slib` artifacts and may invoke the
//! paired `scoopc` process. It deliberately has no dependency on the compiler
//! implementation pipeline.

mod discovery;
mod locator;
mod request;

pub use discovery::*;
pub use locator::{DependencyLocatorError, LocatorIoOperation};
pub use request::*;
