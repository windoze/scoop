//! Immutable editor inputs, ordinary project loading, and query snapshots.

mod cancellation;
mod declarations;
mod host;
mod semantic;
mod syntax;
mod workspace;

pub use cancellation::{Cancellation, Cancelled};
pub use declarations::DeclarationAnalysisError;
pub use host::{AnalysisHost, AnalysisSnapshot, SourceInput};
pub use scoop_source::{Revision, SourceId, SourceVersion};
pub use semantic::{ProjectAnalysisError, SemanticSession};
pub use syntax::{Fold, FoldKind, SyntaxPtr, folding_ranges, selection_ranges};
pub use workspace::{
    ArtifactConeInput, ConeInput, ProjectConfiguration, ProjectDiagnostic, ProjectInput,
    ProjectSource, SourceConeInput, WorkspaceInputs, WorkspaceLoader, normalize_path,
};
