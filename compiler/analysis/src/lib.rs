//! Immutable editor inputs and query snapshots; transport and project loading are callers.

mod cancellation;
mod host;
mod syntax;

pub use cancellation::{Cancellation, Cancelled};
pub use host::{AnalysisHost, AnalysisSnapshot, SourceInput};
pub use scoop_source::{Revision, SourceId, SourceVersion};
pub use syntax::{Fold, FoldKind, SyntaxPtr, folding_ranges, selection_ranges};
