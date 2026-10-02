//! Single-Cone compiler driver and artifact-production pipeline.

mod artifact_production;
mod object_production;
mod request;
mod trusted_core;

pub use artifact_production::{
    CrossConeArtifactMetadataInputV1, CrossConeLayoutArtifactMetadataInputV1,
    LayoutArtifactProductionError,
};
pub use object_production::{
    BuiltinObjectProducerV1, BuiltinObjectProductionError,
    CBridgeEnvelopeVerifiedObjectProductionV1, CodeFingerprintedObjectProductionV1,
    DigestPatchVerifiedObjectProductionV1, FinalizedStrongObjectProductionV1,
    LinkSymbolVerifiedObjectProductionV1, PlannedBuiltinObjectProductionV1,
    RegistrationDependencyFingerprintedProductionV1,
    RegistrationObjectLeafFingerprintedProductionV1, RegistrationObjectVerifiedObjectProductionV1,
    StackmapVerifiedObjectProductionV1, StrongRelocationVerifiedObjectProductionV1,
};
pub use request::{
    BuildRequestNormalizationError, CurrentConeDiagnosticSet, CurrentConeDiagnosticSetError,
    CurrentConeHirStageError, CurrentConeInput, CurrentConeLirStageError, CurrentConeMirStageError,
    CurrentConeOperandError, CurrentConeOperandErrorKind, CurrentConeProductionError,
    CurrentConeProductionFailure, CurrentConeSourceStageError, DiagnosticMappingError,
    DiagnosticOutputPolicy, DirectBuildOptions, EmittedStageDump, ExplicitDependencyArtifactInput,
    ExplicitDependencyInputs, ExplicitDependencyLoadError, ExplicitDependencyLoadOperation,
    ExplicitDependencyRole, ExplicitDependencyValidationError, HostArtifactLocator,
    LoadedCurrentConeInput, LoadedSingleConeBuildRequest, OutputAliasRole,
    OutputIsolationErrorKind, ParsedSingleConeBuildRequest, SingleConeBuildRequest,
    SingleConeBuildRequestError, SingleConeDependencyValidationError, SingleConePreflightError,
    SingleConeProductionError, SingleConeProductionSuccess, SlibOutputDestination, StageDumpKind,
    StageDumpPolicy, TrustedCoreInput, ValidatedCompilerProtocols, ValidatedCurrentConeInput,
    ValidatedExplicitDependencyInputSet, ValidatedSingleConeBuildRequest,
    classify_current_cone_operand, normalize_direct_build_request,
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
