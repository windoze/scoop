//! Profiling data shared by compiler stages and the PGO command.
//!
//! This crate performs no I/O and does not depend on LLVM or lowering stages.

use serde::{Deserialize, Serialize};

mod digest;
mod feedback;
mod instrumentation;
mod mapping;
mod usage;

pub use digest::{BodyId, Fingerprint};
pub use feedback::{Count, Feedback, Frequency, MissingCount};
pub use mapping::{
    BodyKey, BodyLayout, Counter, IndexedRecord, InlineOrigin, Location, Position, PositionKind,
    ProfileIndex, ProfileMap, Record, SCHEMA,
};
pub use usage::{FeedbackConsumer, UseSummary};

/// Whether the produced machine code contains runtime counters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Instrumentation {
    #[default]
    Off,
    Generate,
}

#[cfg(test)]
mod tests;
