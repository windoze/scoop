//! Single-Cone compiler driver and artifact-production pipeline.

mod artifact_production;
mod generated_c;
mod object_production;
mod request;
mod trusted_core;

pub use artifact_production::{
    CrossConeArtifactMetadataInputV1, CrossConeLayoutArtifactMetadataInputV1,
    LayoutArtifactProductionError,
};
pub use object_production::{BuiltinObjectProducerV1, BuiltinObjectProductionError};
pub use request::{
    BuildRequestNormalizationError, CurrentConeDiagnosticSet, CurrentConeDiagnosticSetError,
    CurrentConeHirStageError, CurrentConeInput, CurrentConeLirStageError, CurrentConeMirStageError,
    CurrentConeOperandError, CurrentConeOperandErrorKind, CurrentConeProductionError,
    CurrentConeProductionFailure, CurrentConeSourceStageError, DiagnosticMappingError,
    DiagnosticOutputPolicy, DirectBuildOptions, DocumentationOutputDestination,
    DocumentationProductionError, EmittedStageDump, ExplicitDependencyArtifactInput,
    ExplicitDependencyInputs, ExplicitDependencyLoadError, ExplicitDependencyLoadOperation,
    ExplicitDependencyRole, ExplicitDependencyValidationError, HostArtifactLocator,
    LoadedCurrentConeInput, LoadedSingleConeBuildRequest, OutputAliasRole,
    OutputIsolationErrorKind, ParsedSingleConeBuildRequest, PublishedDocumentation,
    SingleConeBuildRequest, SingleConeBuildRequestError, SingleConeCompilation,
    SingleConeDependencyValidationError, SingleConeDocumentationSuccess, SingleConeOutput,
    SingleConePreflightError, SingleConeProductionError, SingleConeProductionSuccess,
    SlibOutputDestination, StageDumpKind, StageDumpPolicy, StageObservation, TrustedCoreInput,
    ValidatedCompilerProtocols, ValidatedCurrentConeInput, ValidatedExplicitDependencyInputSet,
    ValidatedSingleConeBuildRequest, classify_current_cone_operand, normalize_direct_build_request,
    normalize_protocol_build_request,
};
pub use trusted_core::{
    TrustedCoreSlotError, TrustedCoreSlotErrorKind, TrustedCoreSlotIoOperation,
    resolve_trusted_core_slot,
};

/// Root of the Cargo workspace (the driver crate lives in `compiler/driver`).
#[cfg(test)]
pub(crate) fn workspace_root() -> std::path::PathBuf {
    scoop_toolchain::development_workspace_root()
}

mod lir_pipeline;
