use scoop_identity::{
    CapabilityId, CborIdentityRecord, ConeCoordinate, ConeIdentity, ConeImageSupportRole,
    DefinitionAtomRole, DefinitionAtomSubkey, DigestNodeId, DigestNodeKey, DigestPatchIntentKey,
    DigestSemanticFieldRole, LinkageClass, ObjectDefinitionAtomId, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentSymbolKey, PersistentSymbolRequest,
    PersistentSymbolRequestTable, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_lir::{
    CanonicalLirFoundation, EntryProductionSourceV1, OdrFreeLirFoundation,
    StrongDigestFinalizationPlanV1, StrongExternalLirBridgeSurfaceV1, StrongProductionSectionV1,
    ValidatedLirTargetSelection,
};
use scoop_wire::{DecodeLimits, encode};

use super::*;
use crate::{
    BootstrapManifest, CanonicalSlibArchive, CodeFingerprint, CompatibilityRecord, ConeKind,
    ConeRecord, ConeSourceForm, FingerprintAvailability, HirFingerprint, ManifestSection,
    MemberPurposeSet, MemberStableKey, MetadataEnvelope, MetadataSection, ProducerRecord,
    RuntimeImageFingerprint, SemanticFingerprintRecord, SlibMember, SlibMemberRole,
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
    assert_eq!(sections.identity(), ConeIdentity::CORE);
    assert_eq!(sections.coordinate(), &ConeCoordinate::reserved_core());
    let _ = sections.hir_foundation_wire();
    let _ = sections.mir_foundation_wire();
    let _ = sections.lir_foundation_wire();
    let _ = sections.strong_production_wire();
    let _ = sections.link_identity_closure_wire();
    let _ = sections.production_manifest_wire();
    let checked = sections.validate_identities().unwrap();
    assert_eq!(checked.identity_count(), 16);
    assert_eq!(checked.declared_identity_count(), 15);
    let odr_free = checked.validate_foundation_structure().unwrap();
    assert_eq!(odr_free.identity(), ConeIdentity::CORE);
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
}

#[test]
fn link_section_decode_requires_foundations_and_matching_semantic_fingerprints() {
    let missing = build_artifact(
        vec![production_manifest_section()],
        vec![strong_section(), closure_section()],
        false,
        false,
    );
    assert!(matches!(
        open_graph(&missing).decode_single_cone_link_sections(),
        Err(SingleConeLinkSectionDecodeError::MissingSection {
            location: Some(MetadataLocation::Lir),
            ..
        })
    ));

    let stale = build_artifact(
        vec![production_manifest_section()],
        vec![strong_section(), closure_section()],
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
    build_artifact(manifest_sections, lir_sections, true, false)
}

fn build_artifact(
    manifest_sections: Vec<ManifestSection>,
    lir_sections: Vec<MetadataSection>,
    include_lir_foundation: bool,
    stale_hir_fingerprint: bool,
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
    let hir_sections = vec![
        MetadataSection::new(
            MetadataLocation::Hir,
            hir_identity_foundation_capability(),
            MemberPurposeSet::COMPILE,
            encode(&hir_foundation).unwrap(),
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
    ];
    let mut complete_lir_sections = Vec::new();
    if include_lir_foundation {
        let (foundation, _) = strong_production_fixture(ConeCoordinate::reserved_core());
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
    let members = vec![
        SlibMember::new(
            ConeIdentity::CORE,
            MemberStableKey::HirMetadata,
            SlibMemberRole::HirMetadata,
            encode(&hir_envelope).unwrap(),
        )
        .unwrap(),
        SlibMember::new(
            ConeIdentity::CORE,
            MemberStableKey::MirMetadata,
            SlibMemberRole::MirMetadata,
            encode(&mir_envelope).unwrap(),
        )
        .unwrap(),
        SlibMember::new(
            ConeIdentity::CORE,
            MemberStableKey::LirMetadata,
            SlibMemberRole::LirMetadata,
            encode(&lir_envelope).unwrap(),
        )
        .unwrap(),
    ];
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
        crate::link_object::encoded_link_identity_closure_for_test(),
    )
    .unwrap()
}

pub(crate) fn strong_production() -> StrongProductionSectionV1 {
    strong_production_fixture(ConeCoordinate::reserved_core()).1
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
        ConeCoordinate::reserved_core(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap()
}

fn selection() -> ValidatedLirTargetSelection {
    ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
}
