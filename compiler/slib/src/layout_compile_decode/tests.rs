use scoop_identity::ArtifactCapabilityProfileId;
use scoop_wire::encode;

use super::*;
use crate::{
    ArtifactCapabilityProfile, ArtifactProfileInventoryError, ArtifactProfileView,
    MemberPurposeSet, MetadataLocation, SectionLocation, hir_cross_cone_interface_capability,
    hir_cross_cone_type_semantics_capability, lir_cross_cone_layout_abi_capability,
    lir_cross_cone_layout_link_closure_capability, lir_cross_cone_link_closure_capability,
    lir_cross_cone_param_free_bridge_capability, lir_strong_production_capability,
    lir_strong_production_v2_capability, mir_cross_cone_param_free_bridge_capability,
    mir_cross_cone_type_bridge_capability,
    strong_compile_decode::tests::{
        build_artifact_for_profile, cone, open_graph, required_sections, section,
    },
};

#[test]
fn layout_profile_decodes_every_compile_required_section_atomically() {
    let bytes = layout_artifact(false, None);
    let sections = open_graph(&bytes)
        .decode_cross_cone_layout_compile_sections()
        .unwrap();

    assert_eq!(sections.coordinate(), cone().coordinate());
    assert_eq!(sections.identity(), cone().identity());
    let _ = sections.hir_foundation_wire();
    let _ = sections.hir_core_production_wire();
    let _ = sections.hir_interface_wire();
    let _ = sections.hir_type_semantics_wire();
    let _ = sections.mir_foundation_wire();
    let _ = sections.mir_core_production_wire();
    let _ = sections.mir_cross_cone_bridge_wire();
    let _ = sections.mir_type_bridge_wire();
    let _ = sections.lir_foundation_wire();
    let _ = sections.lir_strong_production_wire();
    let _ = sections.lir_cross_cone_bridge_wire();
    let _ = sections.lir_layout_abi_wire();
}

#[test]
fn layout_profile_validates_all_foundations_before_exposing_new_payloads() {
    let bytes = layout_artifact(false, None);
    let sections = open_graph(&bytes)
        .decode_cross_cone_layout_compile_sections()
        .unwrap()
        .validate_foundation_identities(std::iter::empty())
        .unwrap()
        .validate_foundation_structure()
        .unwrap();

    assert_eq!(sections.coordinate(), cone().coordinate());
    assert_eq!(sections.identity(), cone().identity());
    assert_eq!(sections.identity_count(), 19);
    assert_eq!(sections.declared_identity_count(), 17);
    assert_eq!(
        sections.hir_foundation().as_canonical().counts().odr_groups,
        0
    );
    assert_eq!(
        sections.mir_foundation().as_canonical().counts().odr_groups,
        0
    );
    assert_eq!(
        sections.lir_foundation().as_canonical().counts().odr_groups,
        0
    );
    let _ = sections.hir_core_production_wire();
    let _ = sections.hir_interface_wire();
    let _ = sections.hir_type_semantics_wire();
    let _ = sections.mir_core_production_wire();
    let _ = sections.mir_cross_cone_bridge_wire();
    let _ = sections.mir_type_bridge_wire();
    let _ = sections.lir_strong_production_wire();
    let _ = sections.lir_cross_cone_bridge_wire();
    let _ = sections.lir_layout_abi_wire();
}

#[test]
fn layout_profile_rejects_the_m23_5_profile_before_payloads() {
    let bytes = crate::cross_cone_compile_decode::tests::cross_cone_artifact(
        crate::cross_cone_compile_decode::tests::empty_cross_cone_hir_interface(),
    );

    assert!(matches!(
        open_graph(&bytes).decode_cross_cone_layout_compile_sections(),
        Err(CrossConeLayoutCompileSectionDecodeError::WrongProfile {
            expected,
            actual,
        }) if expected == ArtifactCapabilityProfileId::cross_cone_layout_strong()
            && actual == ArtifactCapabilityProfileId::cross_cone_semantics_strong()
    ));
}

#[test]
fn layout_profile_requires_the_type_semantics_section() {
    let bytes = layout_artifact(false, Some(hir_cross_cone_type_semantics_capability()));

    assert!(matches!(
        open_graph(&bytes).decode_cross_cone_layout_compile_sections(),
        Err(CrossConeLayoutCompileSectionDecodeError::Inventory(
            ArtifactProfileInventoryError::MissingRequiredCapability {
                view: ArtifactProfileView::Compile,
                location: SectionLocation::Hir,
                capability,
            }
        )) if capability == hir_cross_cone_type_semantics_capability()
    ));
}

#[test]
fn layout_profile_rejects_a_noncanonical_new_compile_payload() {
    let bytes = layout_artifact_with_hir_type_payload(vec![0x80], false);

    assert!(matches!(
        open_graph(&bytes).decode_cross_cone_layout_compile_sections(),
        Err(CrossConeLayoutCompileSectionDecodeError::InnerSection {
            location: MetadataLocation::Hir,
            capability,
            ..
        }) if capability == hir_cross_cone_type_semantics_capability()
    ));
}

#[test]
fn layout_profile_rejects_a_semantic_fingerprint_mismatch() {
    let bytes = layout_artifact(true, None);

    assert!(matches!(
        open_graph(&bytes).decode_cross_cone_layout_compile_sections(),
        Err(
            CrossConeLayoutCompileSectionDecodeError::SemanticFingerprintMismatch {
                location: MetadataLocation::Hir,
                ..
            }
        )
    ));
}

fn layout_artifact(
    stale_hir_fingerprint: bool,
    omitted: Option<scoop_identity::CapabilityId>,
) -> Vec<u8> {
    let (mut hir, mir, lir) = layout_sections(empty_type_semantics());
    if let Some(capability) = omitted {
        hir.retain(|section| section.capability() != &capability);
    }
    build_artifact_for_profile(
        cone(),
        ArtifactCapabilityProfile::CROSS_CONE_LAYOUT_STRONG,
        hir,
        mir,
        lir,
        stale_hir_fingerprint,
    )
}

fn layout_artifact_with_hir_type_payload(
    hir_type_semantics: Vec<u8>,
    stale_hir_fingerprint: bool,
) -> Vec<u8> {
    let (hir, mir, lir) = layout_sections(hir_type_semantics);
    build_artifact_for_profile(
        cone(),
        ArtifactCapabilityProfile::CROSS_CONE_LAYOUT_STRONG,
        hir,
        mir,
        lir,
        stale_hir_fingerprint,
    )
}

fn layout_sections(
    hir_type_semantics: Vec<u8>,
) -> (
    Vec<crate::MetadataSection>,
    Vec<crate::MetadataSection>,
    Vec<crate::MetadataSection>,
) {
    let (mut hir, mut mir, mut lir) = required_sections();
    hir.push(section(
        MetadataLocation::Hir,
        hir_cross_cone_interface_capability(),
        MemberPurposeSet::COMPILE,
        crate::cross_cone_compile_decode::tests::empty_cross_cone_hir_interface(),
    ));
    hir.push(section(
        MetadataLocation::Hir,
        hir_cross_cone_type_semantics_capability(),
        MemberPurposeSet::COMPILE,
        hir_type_semantics,
    ));
    mir.push(section(
        MetadataLocation::Mir,
        mir_cross_cone_param_free_bridge_capability(),
        MemberPurposeSet::COMPILE,
        empty_array_fields(2),
    ));
    mir.push(section(
        MetadataLocation::Mir,
        mir_cross_cone_type_bridge_capability(),
        MemberPurposeSet::COMPILE,
        empty_array_fields(7),
    ));

    lir.retain(|section| section.capability() != &lir_strong_production_capability());
    let (_, production) =
        crate::link_decode::strong_production_fixture_for_test(cone().coordinate().clone());
    lir.extend([
        section(
            MetadataLocation::Lir,
            lir_strong_production_v2_capability(),
            MemberPurposeSet::COMPILE_AND_LINK,
            encode(&production).unwrap(),
        ),
        section(
            MetadataLocation::Lir,
            lir_cross_cone_param_free_bridge_capability(),
            MemberPurposeSet::COMPILE,
            empty_array_fields(2),
        ),
        section(
            MetadataLocation::Lir,
            lir_cross_cone_link_closure_capability(),
            MemberPurposeSet::LINK,
            vec![0x80],
        ),
        section(
            MetadataLocation::Lir,
            lir_cross_cone_layout_abi_capability(),
            MemberPurposeSet::COMPILE,
            empty_layout_abi(),
        ),
        section(
            MetadataLocation::Lir,
            lir_cross_cone_layout_link_closure_capability(),
            MemberPurposeSet::LINK,
            vec![0xff],
        ),
    ]);
    (hir, mir, lir)
}

fn empty_type_semantics() -> Vec<u8> {
    empty_array_fields(8)
}

fn empty_array_fields(count: u8) -> Vec<u8> {
    assert!(count <= 15);
    let mut bytes = vec![0xa0 | count];
    for field in 1..=count {
        bytes.extend([field, 0x80]);
    }
    bytes
}

fn empty_layout_abi() -> Vec<u8> {
    let mut bytes = empty_array_fields(5);
    bytes[0] = 0xa6;
    bytes.extend([0x06, 0xa2, 0x01, 0x80, 0x02, 0x80]);
    bytes
}
