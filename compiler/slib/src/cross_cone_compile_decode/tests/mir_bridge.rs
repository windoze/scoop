use super::*;

#[test]
fn cross_cone_hir_front_rejects_a_noncanonical_mir_bridge_payload() {
    let (mut hir, mut mir, mut lir) = required_sections();
    hir.push(section(
        MetadataLocation::Hir,
        hir_cross_cone_interface_capability(),
        MemberPurposeSet::COMPILE,
        empty_cross_cone_hir_interface(),
    ));
    add_cross_cone_bridge_sections(&mut mir, &mut lir);
    let bridge = mir
        .iter_mut()
        .find(|section| section.capability() == &mir_cross_cone_param_free_bridge_capability())
        .unwrap();
    *bridge = section(
        MetadataLocation::Mir,
        mir_cross_cone_param_free_bridge_capability(),
        MemberPurposeSet::COMPILE,
        vec![0x80],
    );
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
        Err(CrossConeHirFrontSectionDecodeError::InnerSection {
            location: MetadataLocation::Mir,
            capability,
            ..
        }) if capability == mir_cross_cone_param_free_bridge_capability()
    ));
}

#[test]
fn cross_cone_hir_front_rejects_a_noncanonical_lir_bridge_payload() {
    let (mut hir, mut mir, mut lir) = required_sections();
    hir.push(section(
        MetadataLocation::Hir,
        hir_cross_cone_interface_capability(),
        MemberPurposeSet::COMPILE,
        empty_cross_cone_hir_interface(),
    ));
    add_cross_cone_bridge_sections(&mut mir, &mut lir);
    let bridge = lir
        .iter_mut()
        .find(|section| section.capability() == &lir_cross_cone_param_free_bridge_capability())
        .unwrap();
    *bridge = section(
        MetadataLocation::Lir,
        lir_cross_cone_param_free_bridge_capability(),
        MemberPurposeSet::COMPILE,
        vec![0x80],
    );
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
        Err(CrossConeHirFrontSectionDecodeError::InnerSection {
            location: MetadataLocation::Lir,
            capability,
            ..
        }) if capability == lir_cross_cone_param_free_bridge_capability()
    ));
}
