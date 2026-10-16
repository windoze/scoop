//! Bind both built-in codegen object sets to canonical `.slib` member identities.

use std::fmt;
use std::path::PathBuf;

use scoop_codegen::{EmittedGeneratedCBridgeObjectSetV1, ProvisionalStrongDigestPatchLocationV1};
use scoop_lir::{
    CBridgeToolchainProfileV1, CanonicalNativeExternalRequirementBuildError, ConeLirFoundation,
    GeneratedBridgeUnitId, LirTargetProfile, ObjectDefinitionPlanId, ProducerUnitPartitionV1,
    ValidatedLirTargetSelection,
};
use scoop_slib::{
    ArtifactCapabilityProfile, BuiltinObjectExternalRequirementClosureError,
    BuiltinObjectSetValidationError, CBridgeTargetSupportRequirementValidationError,
    CanonicalGeneratedBridgeObjectUnitSetV1, CanonicalScoopLirObjectUnitSetV1,
    CodeObjectProjectionError, ConeRecord, CrossConeStrongRequirementValidationError,
    CurrentConeUndefinedRequirementValidationError, DefinedLinkSymbolOwnerBuildError,
    DependencyRecord, DigestPatchSiteValidationError, EntryPatchError,
    GeneratedCBridgeObjectCandidateV1, GeneratedCBridgeSemanticValidationError,
    GeneratedCObjectReadError, LinkObjectMemberSetPlanError, MissingDigestPatchSiteV1,
    ObjectUnitSetError, PlannedGeneratedBridgeObjectMemberV1, PlannedLinkObjectMemberSetV1,
    PlannedScoopLirObjectMemberV1, PlannedStrongObjectSymbolSetV1, ProductionCodeProjectionError,
    ProvisionalDigestPatchSiteV1, RuntimeAndEhRequirementValidationError,
    RuntimeImageFingerprintError, RuntimeImagePatchError, ScoopLirObjectCandidateV1,
    ScoopStackmapReadError, ScoopStackmapSetV1, SlibMember, SlibMemberId, SlibMemberRecordError,
    SourceExternalRequirementValidationError, StrongCallableBodyFingerprintError,
    StrongCallableFingerprintError, StrongImmortalObjectFingerprintError,
    StrongImmortalObjectRegistrationValidationError, StrongInitializationFingerprintError,
    StrongInitializationRegistrationValidationError, StrongObjectSymbolPlanningError,
    StrongRegistrationPatchError, StrongSafepointFingerprintError,
    StrongStaticStorageFingerprintError, StrongStaticStorageRegistrationValidationError,
    StrongStaticStorageShapeFingerprintError, StrongTypeFingerprintError,
    StrongTypeRegistrationValidationError, UndefinedSymbolRequirementFinalizationError,
    VerifiedScoopLirDigestPatchSiteSetV1, bind_callable_registrations_v1, bind_cone_image_v1,
    bind_entry_production_v1, bind_safepoint_registrations_v1,
    compute_strong_callable_fingerprints_v1, compute_strong_immortal_object_fingerprints_v1,
    compute_strong_safepoint_fingerprints_v1, compute_strong_static_storage_fingerprints_v1,
    compute_strong_static_storage_shape_fingerprints_v1, read_generated_c_objects_v1,
    read_scoop_stackmaps_v1, seal_builtin_object_external_requirements_v1,
    verify_builtin_object_strong_relocations_v1, verify_c_bridge_target_support_requirements_v1,
    verify_current_cone_undefined_requirements_v1, verify_generated_c_bridge_semantics_v1,
    verify_runtime_and_eh_requirements_v1, verify_scoop_lir_digest_patch_sites_v1,
    verify_strong_immortal_object_registrations_v1, verify_strong_static_storage_registrations_v1,
};

mod errors;
pub(crate) mod layout;
mod members;
mod planned;
pub use errors::*;
pub use members::BuiltinObjectProducerV1;
use members::*;
pub use planned::*;

#[cfg(test)]
mod tests;
