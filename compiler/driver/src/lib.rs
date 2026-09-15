//! Single-Cone compiler driver and artifact-production pipeline.

use std::path::{Path, PathBuf};

mod artifact_production;
mod ir_production;
mod object_production;
mod request;
mod trusted_core;

pub use artifact_production::{
    AssembledStrongArtifactProductionV1, StrongArtifactMetadataInputV1,
    StrongArtifactProductionError,
};
pub use ir_production::{SingleConeStrongIrProductionV1, StrongIrArtifactProductionError};
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
    BuildRequestNormalizationError, CoreBootstrapHirInputError, CoreBootstrapHirStageError,
    CoreBootstrapLirStageError, CoreBootstrapMirStageError, CoreBootstrapProductionError,
    CoreBootstrapStrongProfileError, CoreOnlyRequestValidationError, CurrentConeDiagnosticSet,
    CurrentConeDiagnosticSetError, CurrentConeInput, CurrentConeOperandError,
    CurrentConeOperandErrorKind, CurrentConeSourceStageError, DiagnosticOutputPolicy,
    EmittedStageDump, ExplicitDependencyArtifactInput, ExplicitDependencyInputs,
    ExplicitDependencyLoadError, ExplicitDependencyLoadOperation, ExplicitDependencyRole,
    ExplicitDependencyValidationError, HostArtifactLocator, LoadedCurrentConeInput,
    LoadedSingleConeBuildRequest, LoadedTrustedCoreInput, NonCoreDependencyInput,
    OrdinaryConeHirOutput, OrdinaryConeHirStageError, OrdinaryConeLirOutput,
    OrdinaryConeLirStageError, OrdinaryConeMirOutput, OrdinaryConeMirStageError,
    OrdinaryConeProductionError, OrdinaryConeStrongProfileError, OrdinaryCoreOnlyHirInputError,
    OutputAliasRole, OutputIsolationErrorKind, ParsedCoreBootstrapBuildRequest,
    ParsedOrdinaryConeBuildRequest, ParsedSingleConeBuildRequest, SingleConeBuildRequest,
    SingleConeBuildRequestError, SingleConePreflightError, SingleConeProductionError,
    SingleConeProductionSuccess, SlibOutputDestination, StageDumpKind, StageDumpPolicy,
    TrustedCoreBootstrapHirInput, TrustedCoreBootstrapHirOutput, TrustedCoreBootstrapLirOutput,
    TrustedCoreBootstrapMirOutput, TrustedCoreInput, ValidatedCoreOnlyBuildRequest,
    ValidatedCurrentConeInput, ValidatedExplicitDependencyInputSet, classify_current_cone_operand,
    normalize_direct_build_request, normalize_protocol_build_request,
};
pub use trusted_core::{
    CoreBootstrapAuthority, LoadedTrustedCoreArtifact, TrustedCoreArtifactAuthority,
    TrustedCoreArtifactAuthorityError, TrustedCoreArtifactInput, TrustedCoreArtifactInputError,
    TrustedCoreArtifactLoadError, TrustedCoreArtifactLoadOperation, TrustedCoreArtifactSlot,
    TrustedCoreArtifactValidationError, TrustedCoreArtifactView, TrustedCoreBootstrapInput,
    TrustedCoreCallableProjectionError, TrustedCoreCallableSetProjectionError,
    TrustedCoreLirSetProjectionError, TrustedCoreSlot, TrustedCoreSlotError,
    TrustedCoreSlotErrorKind, TrustedCoreSlotIoOperation, TrustedCoreSourceSlot,
    ValidatedTrustedCoreArtifact, resolve_trusted_core_slot,
};

/// Root of the Cargo workspace (the driver crate lives in `compiler/driver`).
pub(crate) fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root exists")
}
