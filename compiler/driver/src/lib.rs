//! Single-Cone compiler driver and artifact-production pipeline.

use std::path::{Path, PathBuf};

mod artifact_production;
mod ir_production;
mod object_production;
mod request;
mod trusted_core;

pub use artifact_production::{
    AssembledCrossConeArtifactProductionV1, CrossConeArtifactProductionError,
    CrossConeStrongArtifactMetadataInputV1,
};
pub use ir_production::{CrossConeStrongIrArtifactProductionError, CrossConeStrongIrProductionV1};
pub use object_production::{
    BuiltinObjectProducerV1, BuiltinObjectProductionError,
    CBridgeEnvelopeVerifiedObjectProductionV1, CodeFingerprintedObjectProductionV1,
    CrossConeCodeFingerprintedObjectProductionV1, CrossConeFinalizedStrongObjectProductionV1,
    CrossConeLinkSymbolVerifiedObjectProductionV1,
    CrossConeRegistrationDependencyFingerprintedProductionV1,
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
    CurrentConeProductionFailure, CurrentConeSourceStageError, CurrentConeStrongProfileError,
    DiagnosticOutputPolicy, EmittedStageDump, ExplicitDependencyArtifactInput,
    ExplicitDependencyInputs, ExplicitDependencyLoadError, ExplicitDependencyLoadOperation,
    ExplicitDependencyRole, ExplicitDependencyValidationError, HostArtifactLocator,
    LoadedCurrentConeInput, LoadedSingleConeBuildRequest, OutputAliasRole,
    OutputIsolationErrorKind, ParsedSingleConeBuildRequest, PublishedStrongArtifact,
    PublishedStrongArtifactValidation, SingleConeBuildRequest, SingleConeBuildRequestError,
    SingleConeDependencyValidationError, SingleConePreflightError, SingleConeProductionError,
    SingleConeProductionSuccess, SlibOutputDestination, StageDumpKind, StageDumpPolicy,
    TrustedCoreInput, ValidatedCompilerProtocols, ValidatedCurrentConeInput,
    ValidatedExplicitDependencyInputSet, ValidatedSingleConeBuildRequest,
    classify_current_cone_operand, normalize_direct_build_request,
    normalize_protocol_build_request,
};
pub use trusted_core::{
    TrustedCoreArtifactValidationError, TrustedCoreCallableSetProjectionError,
    TrustedCoreLirSetProjectionError, TrustedCoreSlotError, TrustedCoreSlotErrorKind,
    TrustedCoreSlotIoOperation, ValidatedTrustedCoreArtifact, resolve_trusted_core_slot,
};

/// Root of the Cargo workspace (the driver crate lives in `compiler/driver`).
pub(crate) fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root exists")
}
