//! Dependency-graph orchestration for Scoop builds.
//!
//! This crate may inspect manifests and `.slib` artifacts and may invoke the
//! paired `scoopc` process. It deliberately has no dependency on the compiler
//! implementation pipeline.

mod artifact;
mod cache;
mod child;
mod compiler;
mod diagnostic;
mod discovery;
mod graph;
mod locator;
mod request;
mod runtime_build;
mod schedule;
mod snapshot;

pub use artifact::*;
pub use cache::*;
pub use child::*;
pub(crate) use child::{ChildIoPlan, ProductionSingleConeCompilerRunner, SingleConeCompilerRunner};
pub use compiler::*;
pub use diagnostic::*;
pub use discovery::*;
pub use graph::*;
pub use locator::{DependencyLocatorError, LocatorIoOperation};
pub use request::*;
pub use runtime_build::*;
pub use schedule::*;
pub use snapshot::*;

#[cfg(test)]
mod test_artifacts;
