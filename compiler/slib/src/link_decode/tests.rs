use scoop_identity::{
    CapabilityId, CborIdentityRecord, ConeCoordinate, ConeIdentity, ConeImageSupportRole,
    DefinitionAtomRole, DefinitionAtomSubkey, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, LinkageClass, ObjectDefinitionAtomId, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    AppleClangCompilerIdentityV1, CBridgeProductionSetV1, CBridgeToolchainProfileV1,
    CanonicalLirFoundation, DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1,
    EntryProductionSourceV1, OdrFreeLirFoundation, StrongDigestFinalizationPlanV1,
    StrongObjectSymbolSurfaceV1, StrongProducerUnitPartitionV1, StrongProductionSectionV1,
    ValidatedLirTargetSelection,
};
use scoop_wire::{decode_canonical, encode};

use super::*;
use crate::{
    BootstrapManifest, CanonicalSlibArchive, CodeFingerprint, CompatibilityRecord, ConeKind,
    ConeRecord, ConeSourceForm, DependencyRecord, FingerprintAvailability, HirFingerprint,
    ManifestSection, MemberPurposeSet, MemberStableKey, MetadataEnvelope, MetadataSection,
    PlannedLinkObjectMemberSetV1, PlannedStrongObjectSymbolSetV1, ProducerRecord,
    RuntimeImageFingerprint, SemanticFingerprintRecord, SlibMember, SlibMemberRole,
};

pub(crate) mod layout_link_support;

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
    assert_eq!(
        odr_free.hir_foundation().as_canonical().counts().odr_groups,
        0
    );
    assert_eq!(
        odr_free.mir_foundation().as_canonical().counts().odr_groups,
        0
    );
    assert_eq!(
        odr_free.lir_foundation().as_canonical().counts().odr_groups,
        0
    );

    let validated = odr_free.validate_production().unwrap();
    assert_eq!(validated.identity(), cone().identity());
    assert!(
        validated
            .production()
            .hir()
            .compiler_protocol_definitions()
            .is_none()
    );
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
    assert!(
        leaves
            .callable_registration_objects()
            .fingerprints()
            .is_empty()
    );
    assert!(leaves.type_registration_objects().fingerprints().is_empty());
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
fn strong_graph_rejects_final_object_bytes_that_do_not_reconstruct() {
    let bytes = complete_artifact(true);

    let dependency_owners = Vec::new();
    assert!(matches!(
        open_graph(&bytes)
            .decode_single_cone_link_sections()
            .unwrap()
            .validate_identities()
            .unwrap()
            .validate_foundation_structure()
            .unwrap()
            .validate_production()
            .unwrap()
            .validate_materializations()
            .unwrap()
            .validate_c_bridge_envelopes(&c_bridge_profile())
            .unwrap()
            .validate_builtin_objects()
            .unwrap()
            .validate_digest_patch_sites()
            .unwrap()
            .validate_registration_objects()
            .unwrap()
            .fingerprint_registration_leaves()
            .unwrap()
            .validate_link_symbol_requirements(&dependency_owners, &c_bridge_profile())
            .unwrap()
            .fingerprint_registration_dependencies()
            .unwrap()
            .finalize_strong_objects(),
        Err(StrongLinkObjectFinalizationError::FinalObjectMismatch(
            ReconstructedScoopObjectError::ByteMismatch(_)
        ))
    ));
}

#[test]
fn link_materialization_requires_the_complete_planned_object_directory() {
    let bytes = build_artifact(
        vec![production_manifest_section()],
        vec![strong_section(), closure_section()],
        true,
        true,
        false,
        false,
    );

    assert!(matches!(
        open_graph(&bytes)
            .decode_single_cone_link_sections()
            .unwrap()
            .validate_identities()
            .unwrap()
            .validate_foundation_structure()
            .unwrap()
            .validate_production()
            .unwrap()
            .validate_materializations(),
        Err(StrongLinkMaterializationError::MissingObjectMember(_))
    ));
}

#[test]
fn link_digest_patch_rejects_a_stale_definition_index_projection() {
    let fixture = link_object_fixture();
    let stale_closure = MetadataSection::new(
        MetadataLocation::Lir,
        lir_link_identity_closure_capability(),
        MemberPurposeSet::LINK,
        crate::link_object::encoded_link_identity_closure_without_object_projection_for_test(
            &fixture.plan,
            digest_patch_intent(),
            fixture.plan.scoop_lir_members()[0].member_id(),
            fixture.checked_offset,
        ),
    )
    .unwrap();
    let bytes = artifact(
        vec![production_manifest_section()],
        vec![strong_section(), stale_closure],
    );

    assert!(matches!(
        open_graph(&bytes)
            .decode_single_cone_link_sections()
            .unwrap()
            .validate_identities()
            .unwrap()
            .validate_foundation_structure()
            .unwrap()
            .validate_production()
            .unwrap()
            .validate_materializations()
            .unwrap()
            .validate_c_bridge_envelopes(&c_bridge_profile())
            .unwrap()
            .validate_builtin_objects()
            .unwrap()
            .validate_digest_patch_sites(),
        Err(StrongLinkDigestPatchError::ClosureProjection(
            LinkObjectProjectionValidationError::ProjectionMismatch
        ))
    ));
}

#[test]
fn link_symbol_validation_rejects_a_stale_defined_owner_projection() {
    let fixture = link_object_fixture();
    let stale_closure = MetadataSection::new(
        MetadataLocation::Lir,
        lir_link_identity_closure_capability(),
        MemberPurposeSet::LINK,
        crate::link_object::encoded_link_identity_closure_without_symbol_projection_for_test(
            &fixture.plan,
            &fixture.builtins,
            digest_patch_intent(),
            fixture.plan.scoop_lir_members()[0].member_id(),
            fixture.checked_offset,
        ),
    )
    .unwrap();
    let bytes = artifact(
        vec![production_manifest_section()],
        vec![strong_section(), stale_closure],
    );

    let dependency_owners = Vec::new();
    assert!(matches!(
        open_graph(&bytes)
            .decode_single_cone_link_sections()
            .unwrap()
            .validate_identities()
            .unwrap()
            .validate_foundation_structure()
            .unwrap()
            .validate_production()
            .unwrap()
            .validate_materializations()
            .unwrap()
            .validate_c_bridge_envelopes(&c_bridge_profile())
            .unwrap()
            .validate_builtin_objects()
            .unwrap()
            .validate_digest_patch_sites()
            .unwrap()
            .validate_registration_objects()
            .unwrap()
            .fingerprint_registration_leaves()
            .unwrap()
            .validate_link_symbol_requirements(&dependency_owners, &c_bridge_profile()),
        Err(StrongLinkSymbolRequirementError::ClosureProjection(
            LinkSymbolProjectionValidationError::DefinedSymbols(
                crate::DefinedLinkSymbolOwnerValidationError::ProjectionMismatch
            )
        ))
    ));
}

#[test]
fn link_section_decode_requires_foundations_and_matching_semantic_fingerprints() {
    let missing = build_artifact(
        vec![production_manifest_section()],
        vec![strong_section(), closure_section()],
        true,
        false,
        false,
        true,
    );
    assert!(matches!(
        open_graph(&missing).decode_single_cone_link_sections(),
        Err(SingleConeLinkSectionDecodeError::MissingSection {
            location: Some(MetadataLocation::Lir),
            ..
        })
    ));

    let missing_hir_production = build_artifact(
        vec![production_manifest_section()],
        vec![strong_section(), closure_section()],
        false,
        true,
        false,
        true,
    );
    assert!(matches!(
        open_graph(&missing_hir_production).decode_single_cone_link_sections(),
        Err(SingleConeLinkSectionDecodeError::MissingSection {
            location: Some(MetadataLocation::Hir),
            ..
        })
    ));

    let stale = build_artifact(
        vec![production_manifest_section()],
        vec![strong_section(), closure_section()],
        true,
        true,
        true,
        true,
    );
    assert!(matches!(
        open_graph(&stale).decode_single_cone_link_sections(),
        Err(
            SingleConeLinkSectionDecodeError::SemanticFingerprintMismatch {
                location: MetadataLocation::Hir,
                ..
            }
        )
    ));
}

#[test]
fn link_section_decode_rejects_the_wrong_profile_before_payloads() {
    let artifact =
        crate::IdentityFoundationArtifact::write(crate::IdentityFoundationArtifactInput::new(
            ProducerRecord::new("test").unwrap(),
            cone(),
            selection(),
            &scoop_hir::CanonicalHirFoundation::empty(),
            &scoop_mir::CanonicalMirFoundation::empty(),
            &CanonicalLirFoundation::empty(),
        ))
        .unwrap();
    let graph = open_graph(artifact.as_bytes());
    assert!(matches!(
        graph.decode_single_cone_link_sections(),
        Err(SingleConeLinkSectionDecodeError::WrongProfile { .. })
    ));
}

#[test]
fn link_section_decode_closes_manifest_inventory() {
    let missing = artifact(Vec::new(), vec![strong_section(), closure_section()]);
    assert!(matches!(
        open_graph(&missing).decode_single_cone_link_sections(),
        Err(SingleConeLinkSectionDecodeError::Inventory(
            ArtifactProfileInventoryError::MissingRequiredCapability {
                location: crate::SectionLocation::Manifest,
                ..
            }
        ))
    ));

    let unknown = ManifestSection::new(
        CapabilityId::new("org.scoop-lang.test", "link", 1).unwrap(),
        MemberPurposeSet::LINK,
        Vec::new(),
    )
    .unwrap();
    let unsupported = artifact(
        vec![production_manifest_section(), unknown],
        vec![strong_section(), closure_section()],
    );
    assert!(matches!(
        open_graph(&unsupported).decode_single_cone_link_sections(),
        Err(SingleConeLinkSectionDecodeError::Inventory(
            ArtifactProfileInventoryError::UnsupportedRequiredCapability {
                location: crate::SectionLocation::Manifest,
                ..
            }
        ))
    ));
}

#[test]
fn link_section_decode_closes_lir_inventory_but_keeps_optional_opaque() {
    let missing = artifact(vec![production_manifest_section()], vec![strong_section()]);
    assert!(matches!(
        open_graph(&missing).decode_single_cone_link_sections(),
        Err(SingleConeLinkSectionDecodeError::Inventory(
            ArtifactProfileInventoryError::MissingRequiredCapability {
                location: crate::SectionLocation::Lir,
                ..
            }
        ))
    ));

    let unknown = MetadataSection::new(
        MetadataLocation::Lir,
        CapabilityId::new("org.scoop-lang.test", "link-metadata", 1).unwrap(),
        MemberPurposeSet::LINK,
        Vec::new(),
    )
    .unwrap();
    let unsupported = artifact(
        vec![production_manifest_section()],
        vec![strong_section(), closure_section(), unknown],
    );
    assert!(matches!(
        open_graph(&unsupported).decode_single_cone_link_sections(),
        Err(SingleConeLinkSectionDecodeError::Inventory(
            ArtifactProfileInventoryError::UnsupportedRequiredCapability {
                location: crate::SectionLocation::Lir,
                ..
            }
        ))
    ));

    let optional = MetadataSection::new(
        MetadataLocation::Lir,
        CapabilityId::new("org.scoop-lang.test", "optional-metadata", 1).unwrap(),
        MemberPurposeSet::NONE,
        Vec::new(),
    )
    .unwrap();
    let accepted = artifact(
        vec![production_manifest_section()],
        vec![strong_section(), closure_section(), optional],
    );
    assert!(
        open_graph(&accepted)
            .decode_single_cone_link_sections()
            .is_ok()
    );
}

fn artifact(
    manifest_sections: Vec<ManifestSection>,
    lir_sections: Vec<MetadataSection>,
) -> Vec<u8> {
    build_artifact(manifest_sections, lir_sections, true, true, false, true)
}

pub(super) fn complete_artifact(corrupt_final_image_digest: bool) -> Vec<u8> {
    let mut hir_foundation = scoop_hir::CanonicalHirFoundation::empty();
    hir_foundation
        .set_types(vec![
            scoop_identity::CoreBuiltinNominal::Unit.identity_record(),
            scoop_identity::CoreBuiltinNominal::Any.identity_record(),
        ])
        .unwrap();
    let mut mir_foundation = scoop_mir::CanonicalMirFoundation::empty();
    mir_foundation
        .set_exact_types(vec![
            CborIdentityRecord::from_key(scoop_identity::ExactTypeKey::Nominal(
                scoop_identity::CoreBuiltinNominal::Unit
                    .identity_record()
                    .id(),
            ))
            .unwrap(),
        ])
        .unwrap();
    let (lir_foundation, _) =
        strong_production_fixture(cone().coordinate().clone(), &[ConeIdentity::CORE]);
    let lir_proof =
        OdrFreeLirFoundation::try_new(cone().identity(), lir_foundation.clone()).unwrap();
    let (strong_production, final_objects, defined_symbols, undefined_symbols) =
        finalized_link_object_fixture();
    let image_patch_offset = final_objects
        .runtime_images()
        .fingerprint()
        .image()
        .image_patch()
        .checked_offset();
    let dependencies = vec![
        DependencyRecord::new(
            ConeCoordinate::reserved_core(),
            HirFingerprint::from_array([1; 32]),
            crate::MirFingerprint::from_array([2; 32]),
            crate::LirFingerprint::from_array([3; 32]),
        )
        .unwrap(),
    ];
    let plan = link_object_plan();
    let member_plan = &plan.scoop_lir_members()[0];
    let link_member = SlibMember::new(
        cone().identity(),
        member_plan.stable_key().clone(),
        member_plan.role().clone(),
        final_objects.objects()[0].bytes().to_vec(),
    )
    .unwrap();
    let link_objects =
        crate::verify_code_link_object_members_v1(final_objects, &[link_member.record().clone()])
            .unwrap();
    let code_projection = crate::verify_single_cone_production_code_projection_v1(
        &cone(),
        &dependencies,
        hir_foundation.counts().sources,
        strong_production.clone(),
        link_objects,
    )
    .unwrap();
    let native = CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
        selection().target(),
        &lir_proof,
    )
    .unwrap();
    let code = crate::compute_code_fingerprint_v1(
        code_projection,
        native,
        defined_symbols,
        undefined_symbols,
    )
    .unwrap();
    let closure = crate::LinkIdentityClosureSectionV1::from_verified_code(&code).unwrap();
    let production_manifest = crate::SingleConeProductionManifestV1::from_verified_code(code);
    let link_member = if corrupt_final_image_digest {
        let mut bytes = link_member.payload().to_vec();
        bytes[usize::try_from(image_patch_offset).unwrap()] ^= 1;
        SlibMember::new(
            cone().identity(),
            member_plan.stable_key().clone(),
            member_plan.role().clone(),
            bytes,
        )
        .unwrap()
    } else {
        link_member
    };

    if !corrupt_final_image_digest {
        let hir_proof = scoop_hir::OdrFreeHirFoundation::try_new(hir_foundation).unwrap();
        let mir_proof = scoop_mir::OdrFreeMirFoundation::try_new(mir_foundation).unwrap();
        let hir_production = decode_canonical::<scoop_hir::DecodedCoreBootstrapInterfaceSectionV1>(
            &empty_hir_library_section(),
        )
        .unwrap()
        .validate_against_strong_foundation(cone().identity(), &hir_proof)
        .unwrap();
        let mir_production = scoop_mir::CoreBootstrapBridgeSectionV1::try_new(
            cone().identity(),
            scoop_mir::EntryMirBridgeBranchV1::Library,
            scoop_mir::StrongCallableBridgeSurfaceV1::from_odr_free_foundation(&mir_proof),
        )
        .unwrap();
        return crate::AssembledSingleConeStrongArtifactV1::write(
            crate::SingleConeStrongArtifactInputV1::new(
                ProducerRecord::new("test").unwrap(),
                cone(),
                dependencies,
                &hir_proof,
                &hir_production,
                &mir_proof,
                &mir_production,
                &lir_proof,
                production_manifest,
                vec![link_member],
            ),
        )
        .unwrap()
        .as_bytes()
        .to_vec();
    }

    let hir_sections = vec![
        MetadataSection::new(
            MetadataLocation::Hir,
            hir_identity_foundation_capability(),
            MemberPurposeSet::COMPILE,
            encode(&hir_foundation).unwrap(),
        )
        .unwrap(),
        MetadataSection::new(
            MetadataLocation::Hir,
            hir_core_bootstrap_interface_capability(),
            MemberPurposeSet::COMPILE,
            empty_hir_library_section(),
        )
        .unwrap(),
    ];
    let mir_sections = vec![
        MetadataSection::new(
            MetadataLocation::Mir,
            mir_identity_foundation_capability(),
            MemberPurposeSet::COMPILE,
            encode(&mir_foundation).unwrap(),
        )
        .unwrap(),
        MetadataSection::new(
            MetadataLocation::Mir,
            mir_core_bootstrap_bridge_capability(),
            MemberPurposeSet::COMPILE,
            empty_mir_library_section(),
        )
        .unwrap(),
    ];
    let lir_sections = vec![
        MetadataSection::new(
            MetadataLocation::Lir,
            lir_identity_foundation_capability(),
            MemberPurposeSet::COMPILE,
            encode(&lir_foundation).unwrap(),
        )
        .unwrap(),
        MetadataSection::new(
            MetadataLocation::Lir,
            lir_strong_production_capability(),
            MemberPurposeSet::COMPILE_AND_LINK,
            encode(&strong_production).unwrap(),
        )
        .unwrap(),
        MetadataSection::new(
            MetadataLocation::Lir,
            lir_link_identity_closure_capability(),
            MemberPurposeSet::LINK,
            encode(&closure).unwrap(),
        )
        .unwrap(),
    ];
    let compatibility =
        CompatibilityRecord::new(selection(), ArtifactCapabilityProfile::SINGLE_CONE_STRONG)
            .unwrap();
    let foundation_fingerprints = SemanticFingerprintRecord::from_metadata_sections(
        &compatibility,
        &dependencies,
        &known_sections(&hir_sections),
        &known_sections(&mir_sections),
        &known_sections(&lir_sections),
    )
    .unwrap();
    let semantic = SemanticFingerprintRecord::from_production_manifest(
        foundation_fingerprints.hir(),
        foundation_fingerprints.mir(),
        foundation_fingerprints.lir(),
        &production_manifest,
    );
    let manifest_section = ManifestSection::new(
        manifest_single_cone_production_capability(),
        MemberPurposeSet::LINK,
        encode(&production_manifest).unwrap(),
    )
    .unwrap();
    let mut members = vec![
        SlibMember::new(
            cone().identity(),
            MemberStableKey::HirMetadata,
            SlibMemberRole::HirMetadata,
            encode(&MetadataEnvelope::new(MetadataLocation::Hir, hir_sections).unwrap()).unwrap(),
        )
        .unwrap(),
        SlibMember::new(
            cone().identity(),
            MemberStableKey::MirMetadata,
            SlibMemberRole::MirMetadata,
            encode(&MetadataEnvelope::new(MetadataLocation::Mir, mir_sections).unwrap()).unwrap(),
        )
        .unwrap(),
        SlibMember::new(
            cone().identity(),
            MemberStableKey::LirMetadata,
            SlibMemberRole::LirMetadata,
            encode(&MetadataEnvelope::new(MetadataLocation::Lir, lir_sections).unwrap()).unwrap(),
        )
        .unwrap(),
        link_member,
    ];
    let manifest = BootstrapManifest::new(
        ProducerRecord::new("test").unwrap(),
        compatibility,
        cone(),
        dependencies,
        &members,
        semantic,
        vec![manifest_section],
    )
    .unwrap();
    CanonicalSlibArchive::write_bootstrap(&manifest, std::mem::take(&mut members))
        .unwrap()
        .as_bytes()
        .to_vec()
}

#[test]
fn strong_artifact_writer_is_byte_reproducible() {
    assert_eq!(complete_artifact(false), complete_artifact(false));
}

fn build_artifact(
    manifest_sections: Vec<ManifestSection>,
    lir_sections: Vec<MetadataSection>,
    include_hir_production: bool,
    include_lir_foundation: bool,
    stale_hir_fingerprint: bool,
    include_link_object: bool,
) -> Vec<u8> {
    let mut hir_foundation = scoop_hir::CanonicalHirFoundation::empty();
    hir_foundation
        .set_types(vec![
            scoop_identity::CoreBuiltinNominal::Unit.identity_record(),
            scoop_identity::CoreBuiltinNominal::Any.identity_record(),
        ])
        .unwrap();
    let mut mir_foundation = scoop_mir::CanonicalMirFoundation::empty();
    mir_foundation
        .set_exact_types(vec![
            CborIdentityRecord::from_key(scoop_identity::ExactTypeKey::Nominal(
                scoop_identity::CoreBuiltinNominal::Unit
                    .identity_record()
                    .id(),
            ))
            .unwrap(),
        ])
        .unwrap();
    let mut hir_sections = vec![
        MetadataSection::new(
            MetadataLocation::Hir,
            hir_identity_foundation_capability(),
            MemberPurposeSet::COMPILE,
            encode(&hir_foundation).unwrap(),
        )
        .unwrap(),
    ];
    if include_hir_production {
        hir_sections.push(
            MetadataSection::new(
                MetadataLocation::Hir,
                hir_core_bootstrap_interface_capability(),
                MemberPurposeSet::COMPILE,
                empty_hir_library_section(),
            )
            .unwrap(),
        );
    }
    let mir_sections = vec![
        MetadataSection::new(
            MetadataLocation::Mir,
            mir_identity_foundation_capability(),
            MemberPurposeSet::COMPILE,
            encode(&mir_foundation).unwrap(),
        )
        .unwrap(),
        MetadataSection::new(
            MetadataLocation::Mir,
            mir_core_bootstrap_bridge_capability(),
            MemberPurposeSet::COMPILE,
            empty_mir_library_section(),
        )
        .unwrap(),
    ];
    let mut complete_lir_sections = Vec::new();
    if include_lir_foundation {
        let (foundation, _) =
            strong_production_fixture(cone().coordinate().clone(), &[ConeIdentity::CORE]);
        complete_lir_sections.push(
            MetadataSection::new(
                MetadataLocation::Lir,
                lir_identity_foundation_capability(),
                MemberPurposeSet::COMPILE,
                encode(&foundation).unwrap(),
            )
            .unwrap(),
        );
    }
    complete_lir_sections.extend(lir_sections);

    let compatibility =
        CompatibilityRecord::new(selection(), ArtifactCapabilityProfile::SINGLE_CONE_STRONG)
            .unwrap();
    let dependencies = vec![
        DependencyRecord::new(
            ConeCoordinate::reserved_core(),
            HirFingerprint::from_array([1; 32]),
            crate::MirFingerprint::from_array([2; 32]),
            crate::LirFingerprint::from_array([3; 32]),
        )
        .unwrap(),
    ];
    let semantic = SemanticFingerprintRecord::from_metadata_sections(
        &compatibility,
        &dependencies,
        &known_sections(&hir_sections),
        &known_sections(&mir_sections),
        &known_sections(&complete_lir_sections),
    )
    .unwrap();
    let semantic = SemanticFingerprintRecord::from_validated_digests(
        if stale_hir_fingerprint {
            HirFingerprint::from_array([9; 32])
        } else {
            semantic.hir()
        },
        semantic.mir(),
        semantic.lir(),
        FingerprintAvailability::Available(CodeFingerprint::from_array([4; 32])),
        FingerprintAvailability::Available(RuntimeImageFingerprint::from_array([5; 32])),
    );

    let hir_envelope = MetadataEnvelope::new(MetadataLocation::Hir, hir_sections).unwrap();
    let mir_envelope = MetadataEnvelope::new(MetadataLocation::Mir, mir_sections).unwrap();
    let lir_envelope = MetadataEnvelope::new(MetadataLocation::Lir, complete_lir_sections).unwrap();
    let mut members = vec![
        SlibMember::new(
            cone().identity(),
            MemberStableKey::HirMetadata,
            SlibMemberRole::HirMetadata,
            encode(&hir_envelope).unwrap(),
        )
        .unwrap(),
        SlibMember::new(
            cone().identity(),
            MemberStableKey::MirMetadata,
            SlibMemberRole::MirMetadata,
            encode(&mir_envelope).unwrap(),
        )
        .unwrap(),
        SlibMember::new(
            cone().identity(),
            MemberStableKey::LirMetadata,
            SlibMemberRole::LirMetadata,
            encode(&lir_envelope).unwrap(),
        )
        .unwrap(),
    ];
    if include_link_object {
        let plan = link_object_plan();
        let member = &plan.scoop_lir_members()[0];
        members.push(
            SlibMember::new(
                cone().identity(),
                member.stable_key().clone(),
                member.role().clone(),
                link_object_bytes(),
            )
            .unwrap(),
        );
    }
    let manifest = BootstrapManifest::new(
        ProducerRecord::new("test").unwrap(),
        compatibility,
        cone(),
        dependencies,
        &members,
        semantic,
        manifest_sections,
    )
    .unwrap();
    CanonicalSlibArchive::write_bootstrap(&manifest, members)
        .unwrap()
        .as_bytes()
        .to_vec()
}

fn known_sections(sections: &[MetadataSection]) -> Vec<MetadataSection> {
    sections
        .iter()
        .filter(|section| {
            crate::CapabilityContractRegistry::contract(section.capability()).is_some()
        })
        .cloned()
        .collect()
}

fn open_graph(bytes: &[u8]) -> ValidatedGraphArtifact<'_> {
    crate::DecodedSlibEnvelope::open(bytes, selection())
        .unwrap()
        .validate_graph()
        .unwrap()
}

fn production_manifest_section() -> ManifestSection {
    ManifestSection::new(
        manifest_single_cone_production_capability(),
        MemberPurposeSet::LINK,
        crate::encoded_library_production_manifest_for_test(),
    )
    .unwrap()
}

fn strong_section() -> MetadataSection {
    MetadataSection::new(
        MetadataLocation::Lir,
        lir_strong_production_capability(),
        MemberPurposeSet::COMPILE_AND_LINK,
        encode(&strong_production()).unwrap(),
    )
    .unwrap()
}

fn closure_section() -> MetadataSection {
    let fixture = link_object_fixture();
    MetadataSection::new(
        MetadataLocation::Lir,
        lir_link_identity_closure_capability(),
        MemberPurposeSet::LINK,
        crate::link_object::encoded_link_identity_closure_for_patch_test(
            &fixture.plan,
            &fixture.builtins,
            digest_patch_intent(),
            fixture.plan.scoop_lir_members()[0].member_id(),
            fixture.checked_offset,
        ),
    )
    .unwrap()
}

pub(crate) fn strong_production() -> StrongProductionSectionV1 {
    strong_production_fixture(cone().coordinate().clone(), &[ConeIdentity::CORE]).1
}

pub(crate) fn strong_production_fixture(
    coordinate: ConeCoordinate,
    direct_dependencies: &[ConeIdentity],
) -> (CanonicalLirFoundation, StrongProductionSectionV1) {
    let producer = coordinate.identity().unwrap();
    let definition = CborIdentityRecord::from_key(
        ObjectDefinitionPlanKey::strong(
            producer,
            StrongDefinitionEntity::cone_image(producer),
            StrongDefinitionRole::ImageDescriptor,
        )
        .unwrap(),
    )
    .unwrap();
    let definition_id = definition.id();
    let symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::ImageDescriptor(producer),
        LinkageClass::ConeStrong,
    )
    .unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical.set_definition_plans(vec![definition]).unwrap();
    canonical
        .set_definition_atoms(image_atoms(definition_id))
        .unwrap();
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(vec![symbol]).unwrap());
    let foundation = OdrFreeLirFoundation::try_new(producer, canonical.clone()).unwrap();
    let image_key = DigestNodeKey::runtime_image(producer);
    let image_id = DigestNodeId::from_key(&image_key).unwrap();
    let patch = DigestPatchIntentKey::new(
        image_id,
        definition_id,
        DefinitionAtomRole::Primary,
        DigestSemanticFieldRole::RuntimeImage,
    );
    let image = scoop_lir::DigestNodeV1::new(image_key, Vec::new(), vec![patch]).unwrap();
    let digests = StrongDigestFinalizationPlanV1::new(vec![image], &foundation).unwrap();

    let registrations = scoop_lir::StrongRegistrationProductionSurfaceV1::empty(
        selection().target(),
        &foundation,
        &digests,
    )
    .unwrap();
    assert_ne!(
        producer,
        ConeIdentity::CORE,
        "the link fixture models an ordinary Cone, not the core protocol surface"
    );
    let production = StrongProductionSectionV1::new(
        coordinate,
        direct_dependencies,
        &foundation,
        digests,
        registrations,
        EntryProductionSourceV1::Library,
        &[],
    )
    .unwrap();
    (canonical, production)
}

pub(crate) fn image_atoms(
    plan: ObjectDefinitionPlanId,
) -> Vec<CborIdentityRecord<ObjectDefinitionAtomId, ObjectDefinitionAtomKey>> {
    let mut keys = vec![ObjectDefinitionAtomKey::new(
        plan,
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    )];
    keys.extend(
        [
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::CoordinateGroup,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::CoordinateName,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::CoordinateVersion,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::Dependencies,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::StaticStorages,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::ImmortalObjects,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::InitializationUnits,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::TypeRegistrations,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::Safepoints,
            ),
            (
                DefinitionAtomRole::RuntimeRecord,
                ConeImageSupportRole::Callables,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::ArrayBoundsMessage,
            ),
            (
                DefinitionAtomRole::AddressTakenConstant,
                ConeImageSupportRole::ArraySizeOverflowMessage,
            ),
        ]
        .map(|(role, support)| {
            ObjectDefinitionAtomKey::new(
                plan,
                role,
                DefinitionAtomSubkey::ConeImageSupport(support),
            )
        }),
    );
    keys.into_iter()
        .map(|key| CborIdentityRecord::from_key(key).unwrap())
        .collect()
}

pub(super) fn cone() -> ConeRecord {
    ConeRecord::new(
        ConeCoordinate::new("test", "strong-link", "0.0.0").unwrap(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap()
}

fn empty_hir_library_section() -> Vec<u8> {
    vec![0xa3, 0x02, 0xa1, 0x00, 0x01, 0x03, 0x80, 0x04, 0x80]
}

fn link_object_plan() -> PlannedLinkObjectMemberSetV1 {
    let (canonical, _) =
        strong_production_fixture(cone().coordinate().clone(), &[ConeIdentity::CORE]);
    let foundation = OdrFreeLirFoundation::try_new(cone().identity(), canonical).unwrap();
    let partition = StrongProducerUnitPartitionV1::from_odr_free_foundation(&foundation).unwrap();
    let units = crate::CanonicalScoopLirObjectUnitSetV1::new(
        partition.scoop_lir_definition_plans().to_vec(),
    )
    .unwrap();
    PlannedLinkObjectMemberSetV1::new(&partition, vec![units], Vec::new()).unwrap()
}

struct LinkObjectFixture {
    bytes: Vec<u8>,
    checked_offset: u64,
    plan: PlannedLinkObjectMemberSetV1,
    builtins: crate::VerifiedBuiltinObjectStrongRelocationSetV1,
}

fn link_object_fixture() -> LinkObjectFixture {
    let (canonical, production) =
        strong_production_fixture(cone().coordinate().clone(), &[ConeIdentity::CORE]);
    let foundation = OdrFreeLirFoundation::try_new(cone().identity(), canonical).unwrap();
    let surface = StrongObjectSymbolSurfaceV1::from_odr_free_foundation(&foundation).unwrap();
    let plan = link_object_plan();
    let symbols =
        PlannedStrongObjectSymbolSetV1::new(selection().target(), &surface, &plan).unwrap();
    let (bytes, checked_offset) = crate::link_object::empty_image_object_for_link_decode_test(
        &symbols.members()[0],
        production.image_plan(),
    );
    let bridge_plan = production.generated_bridge_plan().clone();
    let profile = c_bridge_profile();
    let bridge_production =
        CBridgeProductionSetV1::from_generated_bridge_plan(&bridge_plan, &profile);
    let bridge_production = crate::verify_c_bridge_production_envelopes_v1(
        bridge_plan,
        bridge_production,
        &profile,
        &plan,
        &[],
    )
    .unwrap();
    let builtins = crate::verify_builtin_object_strong_relocations_v1(
        &plan,
        &symbols,
        &[crate::ScoopLirObjectCandidateV1::new(
            plan.scoop_lir_members()[0].member_id(),
            &bytes,
        )],
        bridge_production,
        &[],
    )
    .unwrap();
    LinkObjectFixture {
        bytes,
        checked_offset,
        plan,
        builtins,
    }
}

fn finalized_link_object_fixture() -> (
    StrongProductionSectionV1,
    crate::VerifiedEntryPatchSetV1,
    crate::CanonicalDefinedLinkSymbolOwnerSetV1,
    crate::CanonicalUndefinedSymbolRequirementSetV1,
) {
    let fixture = link_object_fixture();
    let (canonical, production) =
        strong_production_fixture(cone().coordinate().clone(), &[ConeIdentity::CORE]);
    let foundation = OdrFreeLirFoundation::try_new(cone().identity(), canonical).unwrap();
    let objects = [crate::ScoopLirObjectCandidateV1::new(
        fixture.plan.scoop_lir_members()[0].member_id(),
        &fixture.bytes,
    )];
    let provisional_sites = [crate::ProvisionalDigestPatchSiteV1::new(
        digest_patch_intent(),
        fixture.plan.scoop_lir_members()[0].member_id(),
        fixture.checked_offset,
        32,
    )];
    let patch_sites = crate::verify_scoop_lir_digest_patch_sites_v1(
        fixture.builtins,
        &foundation,
        production.digest_finalization_plan().clone(),
        &objects,
        &provisional_sites,
    )
    .unwrap();
    let defined_symbols =
        crate::CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(
            patch_sites.builtins().strong_relocations(),
        )
        .unwrap();
    let registrations = production.registration_production();
    let stackmaps = crate::verify_scoop_lir_stackmaps_v1(
        patch_sites.builtins().clone(),
        registrations.safepoint_semantics(),
        &objects,
    )
    .unwrap();
    let safepoint_registrations = crate::verify_strong_safepoint_registrations_v1(
        stackmaps.clone(),
        patch_sites.clone(),
        registrations.safepoints().clone(),
        &objects,
    )
    .unwrap();
    let safepoints =
        crate::compute_strong_safepoint_fingerprints_v1(safepoint_registrations, &objects).unwrap();
    let callable_registrations = crate::verify_strong_callable_registrations_v1(
        patch_sites.clone(),
        registrations.callables().clone(),
        &objects,
    )
    .unwrap();
    let callable_objects = crate::compute_strong_callable_registration_object_fingerprints_v1(
        callable_registrations,
        &objects,
    )
    .unwrap();
    let requirements = empty_undefined_requirements(&patch_sites, &foundation, &production);
    let callable_bodies = crate::compute_strong_callable_body_object_fingerprints_v1(
        callable_objects,
        stackmaps,
        requirements.clone(),
        &objects,
    )
    .unwrap();
    let callables =
        crate::compute_strong_callable_fingerprints_v1(callable_bodies.clone()).unwrap();
    let type_registrations = crate::verify_strong_type_registrations_v1(
        patch_sites.clone(),
        registrations.types().clone(),
        &objects,
    )
    .unwrap();
    let type_objects = crate::compute_strong_type_registration_object_fingerprints_v1(
        type_registrations,
        &objects,
    )
    .unwrap();
    let type_dependencies =
        crate::compute_strong_type_dependency_fingerprints_v1(type_objects, &objects).unwrap();
    let types = crate::compute_strong_type_fingerprints_v1(type_dependencies).unwrap();
    let immortal_registrations = crate::verify_strong_immortal_object_registrations_v1(
        patch_sites.clone(),
        registrations.immortal_objects().clone(),
        &objects,
    )
    .unwrap();
    let immortal_registration_objects =
        crate::compute_strong_immortal_object_registration_object_fingerprints_v1(
            immortal_registrations,
            &objects,
        )
        .unwrap();
    let immortal_definitions = crate::compute_strong_immortal_object_definition_fingerprints_v1(
        immortal_registration_objects,
        requirements.clone(),
        &objects,
    )
    .unwrap();
    let immortal_objects =
        crate::compute_strong_immortal_object_fingerprints_v1(immortal_definitions).unwrap();
    let static_storage_registrations = crate::verify_strong_static_storage_registrations_v1(
        patch_sites.clone(),
        registrations.static_storages().clone(),
        &objects,
    )
    .unwrap();
    let static_storage_objects =
        crate::compute_strong_static_storage_registration_object_fingerprints_v1(
            static_storage_registrations,
            &objects,
        )
        .unwrap();
    let static_storage_definitions =
        crate::compute_strong_static_storage_definition_fingerprints_v1(
            static_storage_objects,
            &objects,
        )
        .unwrap();
    let static_storage_shapes =
        crate::compute_strong_static_storage_shape_fingerprints_v1(static_storage_definitions)
            .unwrap();
    let static_storages =
        crate::compute_strong_static_storage_fingerprints_v1(static_storage_shapes).unwrap();
    let initialization_registrations = crate::verify_strong_initialization_registrations_v1(
        patch_sites.clone(),
        registrations.initialization_units().clone(),
        &objects,
    )
    .unwrap();
    let initialization_objects =
        crate::compute_strong_initialization_registration_object_fingerprints_v1(
            initialization_registrations,
            &objects,
        )
        .unwrap();
    let initialization_definitions =
        crate::compute_strong_initialization_definition_fingerprints_v1(
            initialization_objects,
            &objects,
        )
        .unwrap();
    let initializations = crate::compute_strong_initialization_fingerprints_v1(
        initialization_definitions,
        &callable_bodies,
    )
    .unwrap();
    let image = crate::verify_cone_image_v1(
        patch_sites.clone(),
        production.image_plan().clone(),
        &objects,
    )
    .unwrap();
    let entry =
        crate::verify_entry_production_v1(patch_sites, production.entry_plan().clone(), &objects)
            .unwrap();
    let patched = crate::patch_strong_registration_fingerprints_v1(
        safepoints,
        callables,
        types,
        immortal_objects,
        static_storages,
        initializations,
        &objects,
    )
    .unwrap();
    let compatibility =
        CompatibilityRecord::new(selection(), ArtifactCapabilityProfile::SINGLE_CONE_STRONG)
            .unwrap();
    let image = crate::compute_runtime_image_fingerprint_v1(image, patched, compatibility).unwrap();
    let image = crate::patch_runtime_image_fingerprint_v1(image).unwrap();
    let final_objects = crate::patch_entry_production_v1(image, entry).unwrap();
    (production, final_objects, defined_symbols, requirements)
}

fn link_object_bytes() -> Vec<u8> {
    finalized_link_object_fixture().1.objects()[0]
        .bytes()
        .to_vec()
}

fn empty_undefined_requirements(
    patch_sites: &crate::VerifiedScoopLirDigestPatchSiteSetV1,
    foundation: &OdrFreeLirFoundation,
    production: &StrongProductionSectionV1,
) -> crate::CanonicalUndefinedSymbolRequirementSetV1 {
    let strong = patch_sites.builtins().strong_relocations().clone();
    let current = crate::verify_current_cone_undefined_requirements_v1(
        strong.clone(),
        production.generated_bridge_plan().clone(),
    )
    .unwrap();
    let native = CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
        selection().target(),
        foundation,
    )
    .unwrap();
    let core =
        crate::verify_dependency_strong_requirements_v1(selection().target(), strong, &[]).unwrap();
    let source = crate::verify_source_external_requirements_v1(core, native.clone()).unwrap();
    let runtime = crate::verify_runtime_and_eh_requirements_v1(source, selection()).unwrap();
    let bridge = crate::verify_generated_c_bridge_semantics_v1(
        patch_sites.clone(),
        production.generated_bridge_plan().clone(),
        native,
        &c_bridge_profile(),
    )
    .unwrap();
    let external = crate::verify_c_bridge_target_support_requirements_v1(runtime, bridge).unwrap();
    let external = crate::seal_builtin_object_external_requirements_v1(external).unwrap();
    crate::finalize_undefined_symbol_requirements_v1(current, external).unwrap()
}

fn digest_patch_intent() -> scoop_identity::DigestPatchIntentId {
    strong_production().digest_finalization_plan().nodes()[0].patch_intents()[0].id()
}

fn selection() -> ValidatedLirTargetSelection {
    ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
}

pub(super) fn c_bridge_profile() -> CBridgeToolchainProfileV1 {
    CBridgeToolchainProfileV1::new_darwin_aarch64_apple_clang(
        DarwinCBridgeDeploymentContractV1::new(
            DarwinPackedVersionV1::new(0x000d_0100).unwrap(),
            DarwinPackedVersionV1::new(0x000e_0200).unwrap(),
            Vec::new(),
        )
        .unwrap(),
        AppleClangCompilerIdentityV1::new(21, 0, 0, "clang-2100.1.1.101").unwrap(),
    )
    .unwrap()
}

fn empty_mir_library_section() -> Vec<u8> {
    encode(
        &scoop_mir::CoreBootstrapBridgeSectionV1::try_new(
            cone().identity(),
            scoop_mir::EntryMirBridgeBranchV1::Library,
            scoop_mir::StrongCallableBridgeSurfaceV1::try_new(Vec::new()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
