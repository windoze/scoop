//! Immutable source and artifact inputs prepared for one build graph.

mod prepared;
mod staging;

pub use prepared::*;
pub use scoop_manifest::{ImmutableInputSnapshot, SnapshotFileError, SnapshotIoOperation};
pub use staging::{StagingError, StagingIoOperation};
