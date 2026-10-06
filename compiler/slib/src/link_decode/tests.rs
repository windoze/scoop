use scoop_identity::{
    CapabilityId, CborIdentityRecord, ConeCoordinate, ConeIdentity, ConeImageSupportRole,
    DefinitionAtomRole, DefinitionAtomSubkey, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, LinkageClass, ObjectDefinitionAtomId, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    AppleClangCompilerIdentityV1, CBridgeProductionSetV1, CBridgeToolchainProfileV1,
    CanonicalLirFoundation, ConeLirFoundation, ConeProductionSectionV1,
    DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1, DigestFinalizationPlanV1,
    EntryProductionSourceV1, ObjectSymbolSurfaceV1, ProducerUnitPartitionV1,
    ValidatedLirTargetSelection,
};
use scoop_wire::encode;

use super::*;
use crate::{
    BootstrapManifest, CanonicalSlibArchive, CodeFingerprint, CompatibilityRecord, ConeKind,
    ConeRecord, ConeSourceForm, DependencyRecord, FingerprintAvailability, HirFingerprint,
    ManifestSection, MemberPurposeSet, MemberStableKey, MetadataEnvelope, MetadataSection,
    PlannedLinkObjectMemberSetV1, PlannedStrongObjectSymbolSetV1, ProducerRecord,
    RuntimeImageFingerprint, SemanticFingerprintRecord, SlibMember, SlibMemberRole,
};

pub(crate) mod layout_link_support;

mod archive;
mod extension_members;
mod objects;
mod production;
mod sections;
mod validation;

pub(super) use archive::complete_artifact;
use archive::{artifact, build_artifact};
use objects::{
    finalized_link_object_fixture, link_object_bytes, link_object_fixture, link_object_plan,
};
pub(super) use production::{c_bridge_profile, cone};
use production::{digest_patch_intent, selection};
pub(crate) use production::{image_atoms, strong_production, strong_production_fixture};
use sections::{
    closure_section, empty_hir_library_section, empty_mir_library_section, open_graph,
    production_manifest_section, strong_section,
};

#[test]
fn strong_graph_decodes_all_link_sections_atomically() {
    let bytes = artifact(
        vec![production_manifest_section()],
        vec![strong_section(), closure_section()],
    );
    let sections = open_graph(&bytes)
        .decode_single_cone_link_sections()
        .unwrap();
    assert_eq!(sections.identity(), cone().identity());
    assert_eq!(sections.coordinate(), cone().coordinate());
    let _ = sections.hir_foundation_wire();
    let _ = sections.hir_production_wire();
    let _ = sections.mir_foundation_wire();
    let _ = sections.mir_production_wire();
    let _ = sections.lir_foundation_wire();
    let _ = sections.strong_production_wire();
    let _ = sections.link_identity_closure_wire();
    let _ = sections.production_manifest_wire();
    let checked = sections.validate_identities().unwrap();
    assert_eq!(checked.identity_count(), 19);
    assert_eq!(checked.declared_identity_count(), 17);
    let odr_free = checked.validate_foundation_structure().unwrap();
    assert_eq!(odr_free.identity(), cone().identity());
    assert_eq!(odr_free.declared_identity_count(), 17);
    assert_eq!(odr_free.hir_foundation().counts().odr_groups, 0);
    assert_eq!(odr_free.mir_foundation().counts().odr_groups, 0);
    assert_eq!(
        odr_free.lir_foundation().as_canonical().counts().odr_groups,
        0
    );

    let validated = odr_free.validate_production().unwrap();
    assert_eq!(validated.identity(), cone().identity());
    assert!(validated.production().hir().compiler_protocols().is_none());
    assert!(
        validated
            .production()
            .mir()
            .strong_callable_bridges()
            .initialization_cycle()
            .is_none()
    );

    let _ = validated.link_identity_closure_wire();
    let _ = validated.production_manifest_wire();
    let materialized = validated.validate_materializations().unwrap();
    assert_eq!(materialized.identity(), cone().identity());
    assert_eq!(materialized.scoop_objects().len(), 1);
    assert!(materialized.generated_bridge_objects().is_empty());
    assert_eq!(
        materialized.materializations().member_plan().producer(),
        cone().identity()
    );
    let c_bridge = materialized
        .validate_c_bridge_envelopes(&c_bridge_profile())
        .unwrap();
    assert_eq!(c_bridge.identity(), cone().identity());
    assert!(c_bridge.c_bridge_production().members().is_empty());
    assert!(matches!(
        c_bridge.production_manifest().c_bridge_production(),
        scoop_lir::CBridgeProductionSetV1::NotUsed
    ));
    let builtins = c_bridge.validate_builtin_objects().unwrap();
    assert_eq!(builtins.identity(), cone().identity());
    assert_eq!(
        builtins
            .builtin_objects()
            .strong_relocations()
            .members()
            .len(),
        1
    );
    let patches = builtins.validate_digest_patch_sites().unwrap();
    assert_eq!(patches.identity(), cone().identity());
    assert_eq!(patches.digest_patch_sites().sites().len(), 1);
    assert_eq!(
        patches.digest_patch_sites().sites()[0].intent(),
        digest_patch_intent()
    );
    let registrations = patches.validate_registration_objects().unwrap();
    assert_eq!(registrations.identity(), cone().identity());
    assert!(registrations.stackmaps().records().is_empty());
    assert!(
        registrations
            .safepoint_registrations()
            .registrations()
            .is_empty()
    );
    assert!(
        registrations
            .callable_registrations()
            .registrations()
            .is_empty()
    );
    assert!(
        registrations
            .type_registrations()
            .registrations()
            .is_empty()
    );
    assert!(
        registrations
            .immortal_object_registrations()
            .registrations()
            .is_empty()
    );
    assert!(
        registrations
            .static_storage_registrations()
            .registrations()
            .is_empty()
    );
    assert!(
        registrations
            .initialization_registrations()
            .registrations()
            .is_empty()
    );
    let leaves = registrations.fingerprint_registration_leaves().unwrap();
    assert_eq!(leaves.identity(), cone().identity());
    assert!(leaves.safepoints().fingerprints().is_empty());
    assert!(leaves.callable_registrations().registrations().is_empty());
    assert!(leaves.type_registrations().registrations().is_empty());
    assert!(
        leaves
            .immortal_object_registration_objects()
            .fingerprints()
            .is_empty()
    );
    assert!(
        leaves
            .static_storage_registration_objects()
            .fingerprints()
            .is_empty()
    );
    assert!(
        leaves
            .initialization_registration_objects()
            .fingerprints()
            .is_empty()
    );
    let dependency_owners = Vec::new();
    let symbols = leaves
        .validate_link_symbol_requirements(&dependency_owners, &c_bridge_profile())
        .unwrap();
    assert_eq!(symbols.identity(), cone().identity());
    assert!(!symbols.defined_symbols().owners().is_empty());
    assert!(symbols.undefined_symbols().requirements().is_empty());
    let fingerprints = symbols.fingerprint_registration_dependencies().unwrap();
    assert_eq!(fingerprints.identity(), cone().identity());
    assert!(fingerprints.safepoints().fingerprints().is_empty());
    assert!(fingerprints.callables().fingerprints().is_empty());
    assert!(fingerprints.types().fingerprints().is_empty());
    assert!(fingerprints.immortal_objects().fingerprints().is_empty());
    assert!(fingerprints.static_storages().fingerprints().is_empty());
    assert!(fingerprints.initializations().fingerprints().is_empty());
    let finalized = fingerprints.finalize_strong_objects().unwrap();
    assert_eq!(finalized.identity(), cone().identity());
    assert_eq!(finalized.final_objects().objects().len(), 1);
    assert!(matches!(
        finalized.final_objects().entry().branch(),
        crate::VerifiedEntryProductionBranchV1::Library
    ));
    assert_ne!(
        finalized
            .final_objects()
            .runtime_images()
            .fingerprint()
            .fingerprint()
            .as_array(),
        &[0; 32]
    );
}

#[test]
fn strong_graph_validates_the_complete_final_link_view() {
    let bytes = complete_artifact(false);

    let dependency_owners = Vec::new();
    let artifact = validate_single_cone_strong_link_artifact(
        open_graph(&bytes),
        &dependency_owners,
        &c_bridge_profile(),
    )
    .unwrap();

    assert_eq!(artifact.identity(), cone().identity());
    assert_eq!(
        artifact.production_manifest().code_proof().producer(),
        cone().identity()
    );
    assert_eq!(
        artifact
            .production_manifest()
            .code_proof()
            .production()
            .link_objects()
            .members()
            .len(),
        1
    );
    assert_eq!(
        artifact
            .link_identity_closure()
            .verified_link_objects()
            .members()
            .len(),
        1
    );
}

#[test]
fn canonical_link_artifact_is_byte_reproducible() {
    assert_eq!(complete_artifact(false), complete_artifact(false));
}
