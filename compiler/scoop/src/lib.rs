//! Dependency-graph orchestration for Scoop builds.
//!
//! This crate may inspect manifests and `.slib` artifacts and may invoke the
//! paired `scoopc` process. It deliberately has no dependency on the compiler
//! implementation pipeline.

mod artifact;
mod artifact_link;
mod build;
mod cache;
mod child;
mod compiler;
mod diagnostic;
mod discovery;
mod graph;
mod locator;
mod materialize;
mod program_link;
mod request;
mod runtime_build;
mod schedule;
mod snapshot;

pub use artifact::*;
pub use artifact_link::*;
pub use build::*;
pub use cache::*;
pub use child::*;
pub(crate) use child::{ChildIoPlan, ProductionSingleConeCompilerRunner, SingleConeCompilerRunner};
pub use compiler::*;
pub use diagnostic::*;
pub use discovery::*;
pub use graph::*;
pub use locator::{DependencyLocatorError, LocatorIoOperation};
pub use program_link::read_built_program;
pub use request::*;
pub use runtime_build::*;
pub use schedule::*;
pub use snapshot::*;

#[cfg(test)]
mod test_artifacts;
