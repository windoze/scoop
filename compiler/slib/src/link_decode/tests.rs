use scoop_identity::{
    CapabilityId, CborIdentityRecord, ConeCoordinate, ConeIdentity, ConeImageSupportRole,
    DefinitionAtomRole, DefinitionAtomSubkey, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, LinkageClass, ObjectDefinitionAtomId, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    AppleClangCompilerIdentityV1, CBridgeToolchainProfileV1, CanonicalLirFoundation,
    DarwinCBridgeDeploymentContractV1, DarwinPackedVersionV1, EntryProductionSourceV1,
    OdrFreeLirFoundation, StrongDigestFinalizationPlanV1, StrongExternalLirBridgeSurfaceV1,
    StrongProducerUnitPartitionV1, StrongProductionSectionV1, ValidatedLirTargetSelection,
};
use scoop_wire::{DecodeLimits, encode};

use super::*;
use crate::{
    BootstrapManifest, CanonicalSlibArchive, CodeFingerprint, CompatibilityRecord, ConeKind,
    ConeRecord, ConeSourceForm, FingerprintAvailability, HirFingerprint, ManifestSection,
    MemberPurposeSet, MemberStableKey, MetadataEnvelope, MetadataSection,
    PlannedLinkObjectMemberSetV1, ProducerRecord, RuntimeImageFingerprint,
    SemanticFingerprintRecord, SlibMember, SlibMemberRole, StrongProfileLirProductionError,
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
    assert_eq!(checked.identity_count(), 17);
    assert_eq!(checked.declared_identity_count(), 15);
    let odr_free = checked.validate_foundation_structure().unwrap();
    assert_eq!(odr_free.identity(), cone().identity());
    assert_eq!(odr_free.declared_identity_count(), 15);
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
    let external =
        StrongExternalLirBridgeSurfaceV1::try_new(cone().identity(), Vec::new()).unwrap();
    let validated = odr_free.validate_production(&external).unwrap();
    assert_eq!(validated.identity(), cone().identity());
    assert!(matches!(
        validated.production().hir().core_interface(),
        scoop_hir::CoreHirInterfaceBranchV1::NotCore
    ));
    assert!(matches!(
        validated.production().mir().core_bridge(),
        scoop_mir::CoreMirBridgeBranchV1::NotCore
    ));
    assert_eq!(validated.production().lir().external_bridges(), &external);
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
}

#[test]
fn link_production_rejects_an_external_surface_for_another_producer() {
    let bytes = artifact(
        vec![production_manifest_section()],
        vec![strong_section(), closure_section()],
    );
    let wrong_external =
        StrongExternalLirBridgeSurfaceV1::try_new(ConeIdentity::CORE, Vec::new()).unwrap();
    assert!(matches!(
        open_graph(&bytes)
            .decode_single_cone_link_sections()
            .unwrap()
            .validate_identities()
            .unwrap()
            .validate_foundation_structure()
            .unwrap()
            .validate_production(&wrong_external),
        Err(StrongProfileProductionError::Lir(
            StrongProfileLirProductionError::Production(
                scoop_lir::StrongProductionSectionValidationError::Expected(
                    scoop_lir::StrongProductionSectionBuildError::ExternalBridgeProducer
                )
            )
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
    let external =
        StrongExternalLirBridgeSurfaceV1::try_new(cone().identity(), Vec::new()).unwrap();
    assert!(matches!(
        open_graph(&bytes)
            .decode_single_cone_link_sections()
            .unwrap()
            .validate_identities()
            .unwrap()
            .validate_foundation_structure()
            .unwrap()
            .validate_production(&external)
            .unwrap()
            .validate_materializations(),
        Err(StrongLinkMaterializationError::MissingObjectMember(_))
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
                empty_not_core_library_section(),
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
            empty_not_core_library_section(),
        )
        .unwrap(),
    ];
    let mut complete_lir_sections = Vec::new();
    if include_lir_foundation {
        let (foundation, _) = strong_production_fixture(cone().coordinate().clone());
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
    let semantic = SemanticFingerprintRecord::from_metadata_sections(
        &compatibility,
        &[],
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
                vec![0xaa, 0xbb, 0xcc, 0xdd],
            )
            .unwrap(),
        );
    }
    let manifest = BootstrapManifest::new(
        ProducerRecord::new("test").unwrap(),
        compatibility,
        cone(),
        Vec::new(),
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
    crate::DecodedSlibEnvelope::open(bytes, DecodeLimits::default(), selection())
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
    MetadataSection::new(
        MetadataLocation::Lir,
        lir_link_identity_closure_capability(),
        MemberPurposeSet::LINK,
        crate::link_object::encoded_link_identity_closure_for_member_plan_test(&link_object_plan()),
    )
    .unwrap()
}

pub(crate) fn strong_production() -> StrongProductionSectionV1 {
    strong_production_fixture(cone().coordinate().clone()).1
}

pub(crate) fn strong_production_fixture(
    coordinate: ConeCoordinate,
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
    let external = StrongExternalLirBridgeSurfaceV1::try_new(producer, Vec::new()).unwrap();
    let production = StrongProductionSectionV1::new(
        coordinate,
        &foundation,
        external,
        digests,
        EntryProductionSourceV1::Library,
        &[],
    )
    .unwrap();
    (canonical, production)
}

fn image_atoms(
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

fn cone() -> ConeRecord {
    ConeRecord::new(
        ConeCoordinate::new("test", "strong-link", "0.0.0").unwrap(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap()
}

fn empty_not_core_library_section() -> Vec<u8> {
    vec![
        0xa3, 0x01, 0xa1, 0x00, 0x01, 0x02, 0xa1, 0x00, 0x01, 0x03, 0x80,
    ]
}

fn link_object_plan() -> PlannedLinkObjectMemberSetV1 {
    let (canonical, _) = strong_production_fixture(cone().coordinate().clone());
    let foundation = OdrFreeLirFoundation::try_new(cone().identity(), canonical).unwrap();
    let partition = StrongProducerUnitPartitionV1::from_odr_free_foundation(&foundation).unwrap();
    let units = crate::CanonicalScoopLirObjectUnitSetV1::new(
        partition.scoop_lir_definition_plans().to_vec(),
    )
    .unwrap();
    PlannedLinkObjectMemberSetV1::new(&partition, vec![units], Vec::new()).unwrap()
}

fn selection() -> ValidatedLirTargetSelection {
    ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
}

fn c_bridge_profile() -> CBridgeToolchainProfileV1 {
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
