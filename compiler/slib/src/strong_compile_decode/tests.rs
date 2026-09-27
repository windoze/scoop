use scoop_hir::CanonicalHirFoundation;
use scoop_identity::{
    CapabilityId, CborIdentityRecord, ConeCoordinate, CoreBuiltinNominal, ExactTypeKey, LayoutKey,
    LinkageClass, PersistentSymbolKey, PersistentSymbolRequest, PersistentSymbolRequestTable,
    RepresentationRole, SemanticIdentitySession,
};
use scoop_lir::CanonicalLirFoundation;
use scoop_mir::CanonicalMirFoundation;
use scoop_wire::encode;

use super::*;
use crate::{
    ArtifactProfileInventoryError, ArtifactProfileView, BootstrapManifest, CanonicalSlibArchive,
    CodeFingerprint, CompatibilityRecord, ConeKind, ConeRecord, ConeSourceForm,
    FingerprintAvailability, HirFingerprint, ManifestSection, MemberPurposeSet, MemberStableKey,
    MetadataEnvelope, MetadataSection, ProducerRecord, RuntimeImageFingerprint,
    SemanticFingerprintRecord, SlibMember, SlibMemberRole, hir_core_bootstrap_interface_capability,
    hir_identity_foundation_capability, lir_identity_foundation_capability,
    lir_link_identity_closure_capability, lir_strong_production_capability,
    manifest_single_cone_production_capability, mir_core_bootstrap_bridge_capability,
    mir_identity_foundation_capability,
};

#[test]
fn strong_graph_decodes_all_compile_sections_atomically() {
    let (hir, mir, lir) = required_sections();
    let bytes = artifact(hir, mir, lir);
    let sections = open_graph(&bytes)
        .decode_single_cone_compile_sections()
        .unwrap();
    assert_eq!(sections.identity(), cone().identity());
    assert_eq!(sections.coordinate(), cone().coordinate());
    let _ = sections.hir_foundation_wire();
    let _ = sections.hir_production_wire();
    let _ = sections.mir_foundation_wire();
    let _ = sections.mir_production_wire();
    let _ = sections.lir_foundation_wire();
    let _ = sections.lir_production_wire();
}

#[test]
fn strong_compile_sections_validate_foundation_identities_as_one_transaction() {
    let (hir, mir, lir) = required_sections();
    let bytes = artifact(hir, mir, lir);
    let checked = open_graph(&bytes)
        .decode_single_cone_compile_sections()
        .unwrap()
        .validate_identities()
        .unwrap();
    assert_eq!(checked.identity(), cone().identity());
    assert_eq!(checked.identity_count(), 19);
    assert_eq!(checked.declared_identity_count(), 17);
    let _ = checked.hir_production_wire();
    let _ = checked.mir_production_wire();
    let _ = checked.lir_production_wire();
}

#[test]
fn strong_compile_foundations_validate_structure_and_reject_all_odr() {
    let (hir, mir, lir) = required_sections();
    let bytes = artifact(hir, mir, lir);
    let checked = open_graph(&bytes)
        .decode_single_cone_compile_sections()
        .unwrap()
        .validate_identities()
        .unwrap()
        .validate_foundation_structure()
        .unwrap();
    assert_eq!(checked.identity(), cone().identity());
    assert_eq!(checked.identity_count(), 19);
    assert_eq!(
        checked.hir_foundation().as_canonical().counts().odr_groups,
        0
    );
    assert_eq!(
        checked.mir_foundation().as_canonical().counts().odr_groups,
        0
    );
    assert_eq!(
        checked.lir_foundation().as_canonical().counts().odr_groups,
        0
    );

    let (hir, mir, lir) = sections_with_lir_odr_symbol();
    let bytes = artifact(hir, mir, lir);
    assert!(matches!(
        open_graph(&bytes)
            .decode_single_cone_compile_sections()
            .unwrap()
            .validate_identities()
            .unwrap()
            .validate_foundation_structure(),
        Err(StrongProfileFoundationError::LirOdr(
            scoop_lir::OdrFreeLirFoundationError::NonStrongSymbolRequest {
                linkage: LinkageClass::OdrWeak,
                ..
            }
        ))
    ));
}

#[test]
fn strong_compile_validates_hir_and_mir_production_sections() {
    let (hir, mir, lir) = required_sections();
    let bytes = artifact(hir, mir, lir);
    let validated = open_graph(&bytes)
        .decode_single_cone_compile_sections()
        .unwrap()
        .validate_identities()
        .unwrap()
        .validate_foundation_structure()
        .unwrap()
        .validate_local_production()
        .unwrap();
    assert_eq!(validated.identity(), cone().identity());
    assert!(validated.hir_production().compiler_protocols().is_none());
    assert!(
        validated
            .mir_production()
            .strong_callable_bridges()
            .initialization_cycle()
            .is_none()
    );
}

#[test]
fn strong_compile_closes_manifest_hir_and_mir_output_relation() {
    let (hir, mir, lir) = required_sections();
    let bytes = artifact(hir, mir, lir);
    let validated = open_graph(&bytes)
        .decode_single_cone_compile_sections()
        .unwrap()
        .validate_identities()
        .unwrap()
        .validate_foundation_structure()
        .unwrap()
        .validate_local_production()
        .unwrap()
        .validate_cross_layer()
        .unwrap();
    assert_eq!(validated.identity(), cone().identity());

    let executable = ConeRecord::new(
        cone().coordinate().clone(),
        ConeKind::Executable,
        ConeSourceForm::Manifest,
    )
    .unwrap();
    let (hir, mir, lir) = required_sections();
    let bytes = artifact_for_cone(executable, hir, mir, lir);
    assert!(matches!(
        open_graph(&bytes)
            .decode_single_cone_compile_sections()
            .unwrap()
            .validate_identities()
            .unwrap()
            .validate_foundation_structure()
            .unwrap()
            .validate_local_production()
            .unwrap()
            .validate_cross_layer(),
        Err(StrongProfileRelationError::OutputMismatch)
    ));

    let hir = None;
    let definition = test_cycle_thrower();
    let strong = scoop_mir::StrongCallableBridgeSurfaceV1::try_new(vec![
        scoop_mir::StrongCallableBridgeV1::new(
            scoop_identity::CallableOwner::Function(definition),
            scoop_identity::ExactCallableSignature::new(
                scoop_identity::Effect::Ordinary,
                None,
                vec![],
                scoop_mir::core_unit_exact_type(),
            ),
        ),
    ])
    .unwrap()
    .with_initialization_cycle(definition)
    .unwrap();
    assert_eq!(
        validate_protocol_relation(hir, &strong),
        Err(StrongProfileRelationError::InitializationCycleRoleMismatch)
    );
}

fn test_cycle_thrower() -> scoop_identity::PersistentFunctionId {
    let declaration = scoop_identity::SourceDeclarationKey::function(
        scoop_identity::SourceDeclarationSite::new(
            scoop_identity::ConeIdentity::CORE,
            scoop_identity::PackagePath::root(),
            scoop_identity::DefinitionOwnerChain::top_level(),
            scoop_identity::DeclarationScope::ConeWide,
        )
        .unwrap(),
        scoop_identity::CanonicalIdentifier::new("cycle").unwrap(),
        0,
        None,
        Vec::new(),
    );
    scoop_identity::PersistentFunctionId::from_source_declaration(&declaration).unwrap()
}

#[test]
fn strong_compile_validates_lir_production_from_the_semantic_front() {
    let (hir, mir, lir) = required_sections();
    let bytes = artifact(hir, mir, lir);

    let validated = open_graph(&bytes)
        .decode_single_cone_compile_sections()
        .unwrap()
        .validate_identities()
        .unwrap()
        .validate_foundation_structure()
        .unwrap()
        .validate_local_production()
        .unwrap()
        .validate_cross_layer()
        .unwrap()
        .validate_lir_production()
        .unwrap();
    assert_eq!(validated.identity(), cone().identity());

    assert!(matches!(
        validated.lir_production().entry_plan(),
        scoop_lir::EntryProductionPlanV1::Library
    ));
    assert!(
        validated
            .lir_production()
            .shape_support_plan()
            .closures()
            .is_empty()
    );
    let native = validated.validate_native_boundary().unwrap();
    assert_eq!(native.identity(), cone().identity());
    let mut session = SemanticIdentitySession::new();
    let compiled = native.commit(&mut session).unwrap();
    assert_eq!(compiled.identity(), cone().identity());
    assert_eq!(compiled.hir().origin(), cone().identity());
    assert_eq!(compiled.mir().origin(), cone().identity());
    assert_eq!(compiled.lir().origin(), cone().identity());
    assert_eq!(session.origin_count(), 1);
    assert_eq!(session.entity_count(), 17);
    assert!(compiled.production().hir().compiler_protocols().is_none());
}

#[test]
fn strong_compile_one_shot_entry_returns_the_final_typed_artifact() {
    let (hir, mir, lir) = required_sections();
    let bytes = artifact(hir, mir, lir);

    let mut session = SemanticIdentitySession::new();

    let compiled =
        validate_single_cone_strong_compile_artifact(open_graph(&bytes), &mut session).unwrap();

    assert_eq!(compiled.identity(), cone().identity());

    assert_eq!(session.origin_count(), 1);
    assert_eq!(session.entity_count(), 17);
}

#[test]
fn compile_section_decode_rejects_wrong_profile_before_payloads() {
    let (hir, mir, lir) = crate::strong_compile_decode::tests::required_sections();
    let bytes = crate::strong_compile_decode::tests::build_artifact_for_profile(
        cone(),
        ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
        hir,
        mir,
        lir,
        false,
    );
    assert!(matches!(
        open_graph(&bytes).decode_single_cone_compile_sections(),
        Err(SingleConeCompileSectionDecodeError::WrongProfile { .. })
    ));
}

#[test]
fn compile_section_decode_closes_compile_inventory() {
    let (mut hir, mir, lir) = required_sections();
    hir.retain(|section| section.capability() != &hir_core_bootstrap_interface_capability());
    let missing = artifact(hir, mir, lir);
    assert!(matches!(
        open_graph(&missing).decode_single_cone_compile_sections(),
        Err(SingleConeCompileSectionDecodeError::Inventory(
            ArtifactProfileInventoryError::MissingRequiredCapability {
                view: ArtifactProfileView::Compile,
                location: crate::SectionLocation::Hir,
                ..
            }
        ))
    ));

    let (mut hir, mir, lir) = required_sections();
    hir.push(
        MetadataSection::new(
            MetadataLocation::Hir,
            CapabilityId::new("org.scoop-lang.test", "compile-metadata", 1).unwrap(),
            MemberPurposeSet::COMPILE,
            Vec::new(),
        )
        .unwrap(),
    );
    let unsupported = artifact(hir, mir, lir);
    assert!(matches!(
        open_graph(&unsupported).decode_single_cone_compile_sections(),
        Err(SingleConeCompileSectionDecodeError::Inventory(
            ArtifactProfileInventoryError::UnsupportedRequiredCapability {
                view: ArtifactProfileView::Compile,
                location: crate::SectionLocation::Hir,
                ..
            }
        ))
    ));
}

#[test]
fn compile_section_decode_keeps_link_only_unknown_section_opaque() {
    let (hir, mir, mut lir) = required_sections();
    lir.push(
        MetadataSection::new(
            MetadataLocation::Lir,
            CapabilityId::new("org.scoop-lang.test", "future-link", 1).unwrap(),
            MemberPurposeSet::LINK,
            Vec::new(),
        )
        .unwrap(),
    );
    let bytes = artifact(hir, mir, lir);
    assert!(
        open_graph(&bytes)
            .decode_single_cone_compile_sections()
            .is_ok()
    );
}

#[test]
fn compile_section_decode_rejects_semantic_fingerprint_mismatch() {
    let (hir, mir, lir) = required_sections();
    let bytes = artifact_with_semantic_mismatch(hir, mir, lir);
    assert!(matches!(
        open_graph(&bytes).decode_single_cone_compile_sections(),
        Err(
            SingleConeCompileSectionDecodeError::SemanticFingerprintMismatch {
                location: MetadataLocation::Hir,
                ..
            }
        )
    ));
}

pub(crate) fn required_sections() -> (
    Vec<MetadataSection>,
    Vec<MetadataSection>,
    Vec<MetadataSection>,
) {
    let (lir_foundation, lir_production) = crate::link_decode::strong_production_fixture_for_test(
        cone().coordinate().clone(),
        &[scoop_identity::ConeIdentity::CORE],
    );
    let mut hir_foundation = CanonicalHirFoundation::empty();
    hir_foundation
        .set_types(vec![
            CoreBuiltinNominal::Unit.identity_record(),
            CoreBuiltinNominal::Any.identity_record(),
        ])
        .unwrap();
    let mut mir_foundation = CanonicalMirFoundation::empty();
    mir_foundation
        .set_exact_types(vec![
            CborIdentityRecord::from_key(ExactTypeKey::Nominal(
                CoreBuiltinNominal::Unit.identity_record().id(),
            ))
            .unwrap(),
        ])
        .unwrap();
    let hir = vec![
        section(
            MetadataLocation::Hir,
            hir_identity_foundation_capability(),
            MemberPurposeSet::COMPILE,
            encode(&hir_foundation).unwrap(),
        ),
        section(
            MetadataLocation::Hir,
            hir_core_bootstrap_interface_capability(),
            MemberPurposeSet::COMPILE,
            empty_hir_library_section(),
        ),
    ];
    let mir = vec![
        section(
            MetadataLocation::Mir,
            mir_identity_foundation_capability(),
            MemberPurposeSet::COMPILE,
            encode(&mir_foundation).unwrap(),
        ),
        section(
            MetadataLocation::Mir,
            mir_core_bootstrap_bridge_capability(),
            MemberPurposeSet::COMPILE,
            empty_mir_library_section(),
        ),
    ];
    let lir = vec![
        section(
            MetadataLocation::Lir,
            lir_identity_foundation_capability(),
            MemberPurposeSet::COMPILE,
            encode(&lir_foundation).unwrap(),
        ),
        section(
            MetadataLocation::Lir,
            lir_strong_production_capability(),
            MemberPurposeSet::COMPILE_AND_LINK,
            encode(&lir_production).unwrap(),
        ),
        section(
            MetadataLocation::Lir,
            lir_link_identity_closure_capability(),
            MemberPurposeSet::LINK,
            crate::link_object::encoded_link_identity_closure_for_test(),
        ),
    ];
    (hir, mir, lir)
}

fn sections_with_lir_odr_symbol() -> (
    Vec<MetadataSection>,
    Vec<MetadataSection>,
    Vec<MetadataSection>,
) {
    let (hir, mir, mut lir) = required_sections();
    let exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap()
    .id();
    let layout = CborIdentityRecord::from_key(LayoutKey::new(
        exact,
        selection().target().wire_id(),
        RepresentationRole::ManagedValue,
    ))
    .unwrap();
    let request = PersistentSymbolRequest::new(
        PersistentSymbolKey::Layout(layout.id()),
        LinkageClass::OdrWeak,
    )
    .unwrap();
    let mut foundation = CanonicalLirFoundation::empty();
    foundation.set_layouts(vec![layout]).unwrap();
    foundation.set_symbol_requests(PersistentSymbolRequestTable::new(vec![request]).unwrap());
    let foundation_section = lir
        .iter_mut()
        .find(|section| section.capability() == &lir_identity_foundation_capability())
        .unwrap();
    *foundation_section = section(
        MetadataLocation::Lir,
        lir_identity_foundation_capability(),
        MemberPurposeSet::COMPILE,
        encode(&foundation).unwrap(),
    );
    (hir, mir, lir)
}

pub(crate) fn section(
    location: MetadataLocation,
    capability: CapabilityId,
    purpose: MemberPurposeSet,
    payload: Vec<u8>,
) -> MetadataSection {
    MetadataSection::new(location, capability, purpose, payload).unwrap()
}

fn artifact(
    hir_sections: Vec<MetadataSection>,
    mir_sections: Vec<MetadataSection>,
    lir_sections: Vec<MetadataSection>,
) -> Vec<u8> {
    build_artifact_for_profile(
        cone(),
        ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
        hir_sections,
        mir_sections,
        lir_sections,
        false,
    )
}

fn artifact_with_semantic_mismatch(
    hir_sections: Vec<MetadataSection>,
    mir_sections: Vec<MetadataSection>,
    lir_sections: Vec<MetadataSection>,
) -> Vec<u8> {
    build_artifact_for_profile(
        cone(),
        ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
        hir_sections,
        mir_sections,
        lir_sections,
        true,
    )
}

fn artifact_for_cone(
    cone: ConeRecord,
    hir_sections: Vec<MetadataSection>,
    mir_sections: Vec<MetadataSection>,
    lir_sections: Vec<MetadataSection>,
) -> Vec<u8> {
    build_artifact_for_profile(
        cone,
        ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
        hir_sections,
        mir_sections,
        lir_sections,
        false,
    )
}

pub(crate) fn build_artifact_for_profile(
    cone: ConeRecord,
    profile: ArtifactCapabilityProfile,
    hir_sections: Vec<MetadataSection>,
    mir_sections: Vec<MetadataSection>,
    lir_sections: Vec<MetadataSection>,
    stale_hir_fingerprint: bool,
) -> Vec<u8> {
    let dependencies = fixture_dependencies(&cone, Vec::new());
    build_artifact_for_profile_with_dependencies(
        cone,
        profile,
        dependencies,
        hir_sections,
        mir_sections,
        lir_sections,
        stale_hir_fingerprint,
    )
}

pub(crate) fn fixture_dependencies(
    cone: &ConeRecord,
    mut dependencies: Vec<crate::DependencyRecord>,
) -> Vec<crate::DependencyRecord> {
    if cone.identity() != scoop_identity::ConeIdentity::CORE
        && !dependencies
            .iter()
            .any(|dependency| dependency.identity() == scoop_identity::ConeIdentity::CORE)
    {
        dependencies.push(
            crate::DependencyRecord::new(
                ConeCoordinate::reserved_core(),
                HirFingerprint::from_array([1; 32]),
                crate::MirFingerprint::from_array([2; 32]),
                crate::LirFingerprint::from_array([3; 32]),
            )
            .unwrap(),
        );
        dependencies.sort_unstable_by_key(crate::DependencyRecord::identity);
    }
    dependencies
}

pub(crate) fn build_artifact_for_profile_with_dependencies(
    cone: ConeRecord,
    profile: ArtifactCapabilityProfile,
    dependencies: Vec<crate::DependencyRecord>,
    hir_sections: Vec<MetadataSection>,
    mir_sections: Vec<MetadataSection>,
    lir_sections: Vec<MetadataSection>,
    stale_hir_fingerprint: bool,
) -> Vec<u8> {
    let producer = cone.identity();
    let compatibility = CompatibilityRecord::new(selection(), profile).unwrap();
    let known_hir = known_sections(&hir_sections);
    let known_mir = known_sections(&mir_sections);
    let known_lir = known_sections(&lir_sections);
    let semantic = SemanticFingerprintRecord::from_metadata_sections(
        &compatibility,
        &dependencies,
        &known_hir,
        &known_mir,
        &known_lir,
    )
    .unwrap();
    let semantic = SemanticFingerprintRecord::from_digests(
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
    let members = vec![
        metadata_member(
            producer,
            MetadataLocation::Hir,
            MemberStableKey::HirMetadata,
            SlibMemberRole::HirMetadata,
            hir_sections,
        ),
        metadata_member(
            producer,
            MetadataLocation::Mir,
            MemberStableKey::MirMetadata,
            SlibMemberRole::MirMetadata,
            mir_sections,
        ),
        metadata_member(
            producer,
            MetadataLocation::Lir,
            MemberStableKey::LirMetadata,
            SlibMemberRole::LirMetadata,
            lir_sections,
        ),
    ];
    let production = ManifestSection::new(
        manifest_single_cone_production_capability(),
        MemberPurposeSet::LINK,
        crate::encoded_library_production_manifest_for_test(),
    )
    .unwrap();
    let manifest = BootstrapManifest::new(
        ProducerRecord::new("test").unwrap(),
        compatibility,
        cone,
        dependencies,
        &members,
        semantic,
        vec![production],
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

fn metadata_member(
    producer: scoop_identity::ConeIdentity,
    location: MetadataLocation,
    stable_key: MemberStableKey,
    role: SlibMemberRole,
    sections: Vec<MetadataSection>,
) -> SlibMember {
    let envelope = MetadataEnvelope::new(location, sections).unwrap();
    SlibMember::new(producer, stable_key, role, encode(&envelope).unwrap()).unwrap()
}

pub(crate) fn open_graph(bytes: &[u8]) -> ValidatedGraphArtifact<'_> {
    crate::DecodedSlibEnvelope::open(bytes, selection())
        .unwrap()
        .validate_graph()
        .unwrap()
}

fn empty_hir_library_section() -> Vec<u8> {
    vec![0xa3, 0x02, 0xa1, 0x00, 0x01, 0x03, 0x80, 0x05, 0x80]
}

pub(crate) fn cone() -> ConeRecord {
    cone_named("strong-compile")
}

pub(crate) fn cone_named(name: &str) -> ConeRecord {
    ConeRecord::new(
        ConeCoordinate::new("test", name, "0.0.0").unwrap(),
        ConeKind::Library,
        ConeSourceForm::Manifest,
    )
    .unwrap()
}

fn selection() -> scoop_lir::ValidatedLirTargetSelection {
    scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1
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
