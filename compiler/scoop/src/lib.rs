//! Dependency-graph orchestration for Scoop builds.
//!
//! This crate may inspect manifests and `.slib` artifacts and may invoke the
//! paired `scoopc` process. It deliberately has no dependency on the compiler
//! implementation pipeline.

mod artifact;
mod compiler;
mod discovery;
mod graph;
mod locator;
mod request;
mod snapshot;

pub use artifact::*;
pub use compiler::*;
pub use discovery::*;
pub use graph::*;
pub use locator::{DependencyLocatorError, LocatorIoOperation};
pub use request::*;
pub use snapshot::*;
