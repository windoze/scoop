use scoop_identity::ArtifactCapabilityProfileId;

use super::*;
use crate::{
    ArtifactCapabilityProfile, ArtifactProfileInventoryError, ArtifactProfileView,
    MemberPurposeSet, MetadataLocation, SectionLocation, hir_cross_cone_interface_capability,
    lir_cross_cone_link_closure_capability, lir_cross_cone_param_free_bridge_capability,
    mir_cross_cone_param_free_bridge_capability,
    strong_compile_decode::tests::{
        build_artifact_for_profile, cone, open_graph, required_sections, section,
    },
};

#[test]
fn cross_cone_profile_decodes_the_complete_hir_front() {
    let bytes = cross_cone_artifact(empty_cross_cone_hir_interface());
    let front = open_graph(&bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();

    assert_eq!(front.coordinate(), cone().coordinate());
    assert_eq!(front.identity(), cone().identity());
    let _ = front.hir_foundation_wire();
    let _ = front.hir_core_production_wire();
    let _ = front.hir_interface_wire();
    let _ = front.mir_foundation_wire();
    let _ = front.mir_core_production_wire();
    let _ = front.lir_foundation_wire();
    let _ = front.lir_strong_production_wire();
}

#[test]
fn cross_cone_hir_front_rejects_the_legacy_profile_before_payloads() {
    let (hir, mir, lir) = required_sections();
    let bytes = build_artifact_for_profile(
        cone(),
        ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
        hir,
        mir,
        lir,
        false,
    );

    assert!(matches!(
        open_graph(&bytes).decode_cross_cone_hir_front_sections(),
        Err(CrossConeHirFrontSectionDecodeError::WrongProfile {
            expected,
            actual,
        }) if expected == ArtifactCapabilityProfileId::cross_cone_semantics_strong()
            && actual == ArtifactCapabilityProfileId::single_cone_strong()
    ));
}

#[test]
fn cross_cone_hir_front_requires_the_general_hir_capability() {
    let (hir, mut mir, mut lir) = required_sections();
    add_cross_cone_bridge_sections(&mut mir, &mut lir);
    let bytes = build_artifact_for_profile(
        cone(),
        ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
        hir,
        mir,
        lir,
        false,
    );

    assert!(matches!(
        open_graph(&bytes).decode_cross_cone_hir_front_sections(),
        Err(CrossConeHirFrontSectionDecodeError::Inventory(
            ArtifactProfileInventoryError::MissingRequiredCapability {
                view: ArtifactProfileView::Compile,
                location: SectionLocation::Hir,
                capability,
            }
        )) if capability == hir_cross_cone_interface_capability()
    ));
}

#[test]
fn cross_cone_hir_front_rejects_noncanonical_general_hir_payload() {
    let bytes = cross_cone_artifact(vec![0x80]);

    assert!(matches!(
        open_graph(&bytes).decode_cross_cone_hir_front_sections(),
        Err(CrossConeHirFrontSectionDecodeError::InnerSection {
            location: MetadataLocation::Hir,
            capability,
            ..
        }) if capability == hir_cross_cone_interface_capability()
    ));
}

fn cross_cone_artifact(hir_interface: Vec<u8>) -> Vec<u8> {
    let (mut hir, mut mir, mut lir) = required_sections();
    hir.push(section(
        MetadataLocation::Hir,
        hir_cross_cone_interface_capability(),
        MemberPurposeSet::COMPILE,
        hir_interface,
    ));
    add_cross_cone_bridge_sections(&mut mir, &mut lir);
    build_artifact_for_profile(
        cone(),
        ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
        hir,
        mir,
        lir,
        false,
    )
}

fn add_cross_cone_bridge_sections(
    mir: &mut Vec<crate::MetadataSection>,
    lir: &mut Vec<crate::MetadataSection>,
) {
    mir.push(section(
        MetadataLocation::Mir,
        mir_cross_cone_param_free_bridge_capability(),
        MemberPurposeSet::COMPILE,
        vec![0x80],
    ));
    lir.push(section(
        MetadataLocation::Lir,
        lir_cross_cone_param_free_bridge_capability(),
        MemberPurposeSet::COMPILE,
        vec![0x80],
    ));
    lir.push(section(
        MetadataLocation::Lir,
        lir_cross_cone_link_closure_capability(),
        MemberPurposeSet::LINK,
        vec![0x80],
    ));
}

fn empty_cross_cone_hir_interface() -> Vec<u8> {
    vec![
        0xaa, 0x01, 0x80, 0x02, 0x80, 0x03, 0x80, 0x04, 0x80, 0x05, 0x80, 0x06, 0x80, 0x07, 0x80,
        0x08, 0x80, 0x09, 0x80, 0x0a, 0x80,
    ]
}
