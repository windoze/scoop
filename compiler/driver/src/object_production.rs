//! Bind both built-in codegen object sets to canonical `.slib` member identities.

use std::fmt;
use std::path::PathBuf;

use scoop_codegen::{
    EmittedGeneratedCBridgeObjectSetV1, EmittedStrongObjectMemberKindV1, EmittedStrongObjectSetV1,
    ProvisionalStrongDigestPatchLocationV1,
};
use scoop_lir::{
    CBridgeProductionSetV1, CBridgeToolchainProfileV1,
    CanonicalNativeExternalRequirementBuildError, GeneratedBridgeUnitId, LirTargetProfile,
    ObjectDefinitionPlanId, OdrFreeLirFoundation, StrongProducerUnitPartitionV1,
    StrongProductionSectionV1, ValidatedLirTargetSelection,
};
use scoop_slib::{
    ArtifactCapabilityProfile, BuiltinObjectExternalRequirementClosureError,
    BuiltinObjectSetValidationError, CBridgeProductionEnvelopeValidationError,
    CBridgeTargetSupportRequirementValidationError, CanonicalDefinedLinkSymbolOwnerSetV1,
    CanonicalGeneratedBridgeObjectUnitSetV1, CanonicalScoopLirObjectUnitSetV1,
    CanonicalUndefinedSymbolRequirementSetV1, CodeFingerprintError,
    CodeLinkObjectMemberValidationError, ConeImageValidationError, ConeRecord,
    CrossConeStrongRequirementValidationError, CurrentConeUndefinedRequirementValidationError,
    DefinedLinkSymbolOwnerBuildError, DependencyRecord, DigestPatchSiteValidationError,
    EntryPatchError, EntryProductionValidationError, GeneratedCBridgeObjectCandidateV1,
    GeneratedCBridgeSemanticValidationError, LinkObjectMemberSetPlanError, ObjectUnitSetError,
    PlannedGeneratedBridgeObjectMemberV1, PlannedLinkObjectMemberSetV1,
    PlannedScoopLirObjectMemberV1, PlannedStrongObjectSymbolSetV1, ProductionCodeProjectionError,
    ProvisionalDigestPatchSiteV1, RuntimeAndEhRequirementValidationError,
    RuntimeImageFingerprintError, RuntimeImagePatchError, ScoopLirObjectCandidateV1,
    ScoopLirStackmapValidationError, SingleConeProductionManifestV1, SlibMember, SlibMemberId,
    SlibMemberRecordError, SourceExternalRequirementValidationError,
    StrongCallableBodyFingerprintError, StrongCallableFingerprintError,
    StrongCallableRegistrationObjectFingerprintError, StrongCallableRegistrationValidationError,
    StrongImmortalObjectDefinitionFingerprintError, StrongImmortalObjectFingerprintError,
    StrongImmortalObjectRegistrationObjectFingerprintError,
    StrongImmortalObjectRegistrationValidationError,
    StrongInitializationDefinitionFingerprintError, StrongInitializationFingerprintError,
    StrongInitializationRegistrationObjectFingerprintError,
    StrongInitializationRegistrationValidationError, StrongObjectSymbolPlanningError,
    StrongRegistrationPatchError, StrongSafepointFingerprintError,
    StrongSafepointRegistrationValidationError, StrongStaticStorageDefinitionFingerprintError,
    StrongStaticStorageFingerprintError, StrongStaticStorageRegistrationObjectFingerprintError,
    StrongStaticStorageRegistrationValidationError, StrongStaticStorageShapeFingerprintError,
    StrongTypeDependencyFingerprintError, StrongTypeFingerprintError,
    StrongTypeRegistrationObjectFingerprintError, StrongTypeRegistrationValidationError,
    UndefinedSymbolRequirementFinalizationError, VerifiedBuiltinObjectStrongRelocationSetV1,
    VerifiedCBridgeProductionEnvelopeSetV1, VerifiedEntryPatchSetV1,
    VerifiedScoopLirDigestPatchSiteSetV1, VerifiedScoopLirStackmapSetV1,
    VerifiedStrongCallableFingerprintSetV1,
    VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
    VerifiedStrongCallableRegistrationSetV1, VerifiedStrongImmortalObjectFingerprintSetV1,
    VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
    VerifiedStrongImmortalObjectRegistrationSetV1, VerifiedStrongInitializationFingerprintSetV1,
    VerifiedStrongInitializationRegistrationObjectFingerprintSetV1,
    VerifiedStrongInitializationRegistrationSetV1, VerifiedStrongSafepointFingerprintSetV1,
    VerifiedStrongSafepointRegistrationSetV1, VerifiedStrongStaticStorageFingerprintSetV1,
    VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1,
    VerifiedStrongStaticStorageRegistrationSetV1, VerifiedStrongTypeFingerprintSetV1,
    VerifiedStrongTypeRegistrationObjectFingerprintSetV1, VerifiedStrongTypeRegistrationSetV1,
    compute_code_fingerprint_v1, compute_runtime_image_fingerprint_v1,
    compute_strong_callable_body_object_fingerprints_v1, compute_strong_callable_fingerprints_v1,
    compute_strong_callable_registration_object_fingerprints_v1,
    compute_strong_immortal_object_definition_fingerprints_v1,
    compute_strong_immortal_object_fingerprints_v1,
    compute_strong_immortal_object_registration_object_fingerprints_v1,
    compute_strong_initialization_definition_fingerprints_v1,
    compute_strong_initialization_fingerprints_v1,
    compute_strong_initialization_registration_object_fingerprints_v1,
    compute_strong_safepoint_fingerprints_v1,
    compute_strong_static_storage_definition_fingerprints_v1,
    compute_strong_static_storage_fingerprints_v1,
    compute_strong_static_storage_registration_object_fingerprints_v1,
    compute_strong_static_storage_shape_fingerprints_v1,
    compute_strong_type_dependency_fingerprints_v1, compute_strong_type_fingerprints_v1,
    compute_strong_type_registration_object_fingerprints_v1,
    finalize_undefined_symbol_requirements_v1, patch_entry_production_v1,
    patch_runtime_image_fingerprint_v1, patch_strong_registration_fingerprints_v1,
    seal_builtin_object_external_requirements_v1, verify_builtin_object_strong_relocations_v1,
    verify_c_bridge_production_envelopes_v1, verify_c_bridge_target_support_requirements_v1,
    verify_code_link_object_members_v1, verify_cone_image_v1,
    verify_current_cone_undefined_requirements_v1, verify_dependency_strong_requirements_v1,
    verify_entry_production_v1, verify_generated_c_bridge_semantics_v1,
    verify_runtime_and_eh_requirements_v1, verify_scoop_lir_digest_patch_sites_v1,
    verify_scoop_lir_stackmaps_v1, verify_single_cone_production_code_projection_v1,
    verify_source_external_requirements_v1, verify_strong_callable_registrations_v1,
    verify_strong_immortal_object_registrations_v1, verify_strong_initialization_registrations_v1,
    verify_strong_safepoint_registrations_v1, verify_strong_static_storage_registrations_v1,
    verify_strong_type_registrations_v1,
};

mod errors;
mod fingerprint_pipeline;
pub(crate) mod layout;
mod members;
mod planned;
mod registrations;
mod verification;
pub use errors::*;
pub use fingerprint_pipeline::*;
pub use members::BuiltinObjectProducerV1;
use members::*;
pub use planned::*;
pub use registrations::*;
pub use verification::*;

#[cfg(test)]
mod tests;
