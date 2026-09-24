use scoop_identity::{ArtifactCapabilityProfileId, CapabilityId};
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
fn layout_link_front_decodes_the_complete_profile_atomically() {
    let bytes = layout_artifact(false, None, empty_link_closure(0x22), false);
    let sections = open_graph(&bytes)
        .decode_cross_cone_layout_link_sections()
        .unwrap();

    assert_eq!(sections.coordinate(), cone().coordinate());
    assert_eq!(sections.identity(), cone().identity());
    assert!(sections.direct_dependencies().is_empty());
    let _ = sections.artifact_fingerprint();
    let _ = sections.decode_usage();
    let _ = sections.production_manifest_wire();
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
    let _ = sections.link_identity_closure_wire();
    let _ = sections.cross_cone_link_closure_wire();
    let _ = sections.layout_link_closure_wire();
}

#[test]
fn layout_link_front_rejects_the_m23_5_profile_before_payloads() {
    let bytes = crate::cross_cone_compile_decode::tests::cross_cone_artifact(
        crate::cross_cone_compile_decode::tests::empty_cross_cone_hir_interface(),
    );

    assert!(matches!(
        open_graph(&bytes).decode_cross_cone_layout_link_sections(),
        Err(CrossConeLayoutLinkSectionDecodeError::WrongProfile { actual })
            if actual == ArtifactCapabilityProfileId::cross_cone_semantics_strong()
    ));
}

#[test]
fn layout_link_front_requires_the_new_link_closure() {
    let bytes = layout_artifact(
        false,
        Some(lir_cross_cone_layout_link_closure_capability()),
        empty_link_closure(0x22),
        false,
    );

    assert!(matches!(
        open_graph(&bytes).decode_cross_cone_layout_link_sections(),
        Err(CrossConeLayoutLinkSectionDecodeError::Inventory(
            ArtifactProfileInventoryError::MissingRequiredCapability {
                view: ArtifactProfileView::Link,
                location: SectionLocation::Lir,
                capability,
            }
        )) if capability == lir_cross_cone_layout_link_closure_capability()
    ));
}

#[test]
fn layout_link_front_rejects_a_malformed_new_link_payload() {
    let bytes = layout_artifact(false, None, vec![0xff], false);

    assert!(matches!(
        open_graph(&bytes).decode_cross_cone_layout_link_sections(),
        Err(CrossConeLayoutLinkSectionDecodeError::InnerSection {
            location: Some(MetadataLocation::Lir),
            capability,
            ..
        }) if capability == lir_cross_cone_layout_link_closure_capability()
    ));
}

#[test]
fn layout_link_front_rejects_a_semantic_fingerprint_mismatch() {
    let bytes = layout_artifact(true, None, empty_link_closure(0x22), false);

    assert!(matches!(
        open_graph(&bytes).decode_cross_cone_layout_link_sections(),
        Err(
            CrossConeLayoutLinkSectionDecodeError::SemanticFingerprintMismatch {
                location: MetadataLocation::Hir,
                ..
            }
        )
    ));
}

#[test]
fn layout_link_front_rejects_mixed_strong_production_versions() {
    let bytes = layout_artifact(false, None, empty_link_closure(0x22), true);

    assert!(matches!(
        open_graph(&bytes).decode_cross_cone_layout_link_sections(),
        Err(CrossConeLayoutLinkSectionDecodeError::Inventory(
            ArtifactProfileInventoryError::ConflictingCapabilityVersion {
                location: SectionLocation::Lir,
                required,
                actual,
                ..
            }
        )) if *required == lir_strong_production_v2_capability()
            && *actual == lir_strong_production_capability()
    ));
}

fn layout_artifact(
    stale_hir_fingerprint: bool,
    omitted: Option<CapabilityId>,
    layout_link_payload: Vec<u8>,
    include_v1_production: bool,
) -> Vec<u8> {
    let (mut hir, mut mir, mut lir) = layout_sections(layout_link_payload);
    if let Some(capability) = omitted {
        hir.retain(|item| item.capability() != &capability);
        mir.retain(|item| item.capability() != &capability);
        lir.retain(|item| item.capability() != &capability);
    }
    if include_v1_production {
        let (_, production) = crate::link_decode::strong_production_fixture_for_test(
            cone().coordinate().clone(),
            &[],
        );
        lir.push(section(
            MetadataLocation::Lir,
            lir_strong_production_capability(),
            MemberPurposeSet::COMPILE_AND_LINK,
            encode(&production).unwrap(),
        ));
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

fn layout_sections(
    layout_link_payload: Vec<u8>,
) -> (
    Vec<crate::MetadataSection>,
    Vec<crate::MetadataSection>,
    Vec<crate::MetadataSection>,
) {
    let (mut hir, mut mir, mut lir) = required_sections();
    hir.extend([
        section(
            MetadataLocation::Hir,
            hir_cross_cone_interface_capability(),
            MemberPurposeSet::COMPILE,
            crate::cross_cone_compile_decode::tests::empty_cross_cone_hir_interface(),
        ),
        section(
            MetadataLocation::Hir,
            hir_cross_cone_type_semantics_capability(),
            MemberPurposeSet::COMPILE,
            empty_array_fields(8),
        ),
    ]);
    mir.extend([
        section(
            MetadataLocation::Mir,
            mir_cross_cone_param_free_bridge_capability(),
            MemberPurposeSet::COMPILE,
            empty_array_fields(2),
        ),
        section(
            MetadataLocation::Mir,
            mir_cross_cone_type_bridge_capability(),
            MemberPurposeSet::COMPILE,
            empty_array_fields(7),
        ),
    ]);

    lir.retain(|item| item.capability() != &lir_strong_production_capability());
    let (_, production) =
        crate::link_decode::strong_production_fixture_for_test(cone().coordinate().clone(), &[]);
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
            empty_link_closure(0x11),
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
            layout_link_payload,
        ),
    ]);
    (hir, mir, lir)
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

fn empty_link_closure(digest_byte: u8) -> Vec<u8> {
    let mut bytes = vec![
        0xa3, 0x01, 0x80, 0x02, 0x80, 0x03, 0xa2, 0x01, 0x80, 0x02, 0x58, 0x20,
    ];
    bytes.extend([digest_byte; 32]);
    bytes
}
