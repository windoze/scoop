//! Immutable editor inputs, ordinary project loading, and query snapshots.

mod cancellation;
mod host;
mod syntax;
mod workspace;

pub use cancellation::{Cancellation, Cancelled};
pub use host::{AnalysisHost, AnalysisSnapshot, SourceInput};
pub use scoop_source::{Revision, SourceId, SourceVersion};
pub use syntax::{Fold, FoldKind, SyntaxPtr, folding_ranges, selection_ranges};
pub use workspace::{
    ArtifactConeInput, ConeInput, ProjectConfiguration, ProjectDiagnostic, ProjectInput,
    ProjectSource, SourceConeInput, WorkspaceInputs, WorkspaceLoader, normalize_path,
};
