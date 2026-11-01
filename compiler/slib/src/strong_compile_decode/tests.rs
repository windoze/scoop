use scoop_hir::CanonicalHirFoundation;
use scoop_identity::{
    CapabilityId, CborIdentityRecord, ConeCoordinate, CoreBuiltinNominal, ExactTypeKey,
};
use scoop_mir::CanonicalMirFoundation;
use scoop_wire::encode;

use super::*;
use crate::{
    ArtifactCapabilityProfile, BootstrapManifest, CanonicalSlibArchive, CodeFingerprint,
    CompatibilityRecord, ConeKind, ConeRecord, ConeSourceForm, FingerprintAvailability,
    HirFingerprint, MemberPurposeSet, MemberStableKey, MetadataEnvelope, MetadataLocation,
    MetadataSection, ProducerRecord, RuntimeImageFingerprint, SemanticFingerprintRecord,
    SlibMember, SlibMemberRole, hir_core_bootstrap_interface_capability,
    hir_identity_foundation_capability, lir_identity_foundation_capability,
    lir_link_identity_closure_capability, lir_strong_production_capability,
    mir_core_bootstrap_bridge_capability, mir_identity_foundation_capability,
};

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
        .set_types(vec![CoreBuiltinNominal::Unit.identity_record()])
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
            MemberPurposeSet::COMPILE_AND_LINK,
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
            MemberPurposeSet::COMPILE_AND_LINK,
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
            MemberPurposeSet::COMPILE_AND_LINK,
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

pub(crate) fn section(
    location: MetadataLocation,
    capability: CapabilityId,
    purpose: MemberPurposeSet,
    payload: Vec<u8>,
) -> MetadataSection {
    MetadataSection::new(location, capability, purpose, payload).unwrap()
}

pub(crate) fn build_artifact_for_profile(
    cone: ConeRecord,
    profile: ArtifactCapabilityProfile,
    hir_sections: Vec<MetadataSection>,
    mir_sections: Vec<MetadataSection>,
    lir_sections: Vec<MetadataSection>,
) -> Vec<u8> {
    let dependencies = fixture_dependencies(&cone, Vec::new());
    build_artifact_for_profile_with_dependencies(
        cone,
        profile,
        dependencies,
        hir_sections,
        mir_sections,
        lir_sections,
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
        semantic.hir(),
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
    let manifest = BootstrapManifest::new(
        ProducerRecord::new("test").unwrap(),
        compatibility,
        cone,
        dependencies,
        &members,
        semantic,
        Vec::new(),
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
    scoop_target_data::DARWIN_AARCH64_LLVM_22_1
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
