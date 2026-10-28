use super::*;

#[test]
fn source_interface_v82_preserves_aggregate_const_values() {
    assert_retired_version(
        hir_cross_cone_interface_capability(),
        82,
        MemberPurposeSet::COMPILE,
        &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC],
    );
}

#[test]
fn type_semantics_v30_preserves_annotation_queries() {
    assert_retired_version(
        hir_cross_cone_type_semantics_capability(),
        30,
        MemberPurposeSet::COMPILE,
        &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC],
    );
}

#[test]
fn mir_type_bridge_v19_preserves_maybe_uninit_representation() {
    assert_retired_version(
        mir_cross_cone_type_bridge_capability(),
        19,
        MemberPurposeSet::COMPILE,
        &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC],
    );
}

#[test]
fn mir_foundation_v8_preserves_annotation_description_owners() {
    assert_retired_version(
        mir_identity_foundation_capability(),
        8,
        MemberPurposeSet::COMPILE_AND_LINK,
        &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC],
    );
}

#[test]
fn lir_layout_abi_v17_preserves_interface_table_projections() {
    assert_retired_version(
        lir_cross_cone_layout_abi_capability(),
        17,
        MemberPurposeSet::COMPILE_AND_LINK,
        &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC],
    );
}

#[test]
fn lir_param_free_v3_retains_target_coercions() {
    assert_retired_version(
        lir_cross_cone_param_free_bridge_capability(),
        3,
        MemberPurposeSet::COMPILE_AND_LINK,
        &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC],
    );
}

#[test]
fn compiler_protocol_v16_preserves_type_info_queries() {
    assert_retired_version(
        hir_core_bootstrap_interface_capability(),
        16,
        MemberPurposeSet::COMPILE,
        &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC],
    );
}

fn assert_retired_version(
    current: CapabilityId,
    major: u32,
    required_for: MemberPurposeSet,
    profiles: &[ArtifactCapabilityProfile],
) {
    let contract = CapabilityContractRegistry::contract(&current).unwrap();
    assert_eq!(current.major_version(), major);
    assert_eq!(contract.required_for(), required_for);
    for retired in 1..major {
        let old = CapabilityId::new(current.namespace(), current.name(), retired).unwrap();
        assert!(CapabilityContractRegistry::contract(&old).is_none());
        for profile in profiles {
            let descriptor = profile.descriptor();
            let inventory = match contract.location() {
                SectionLocation::Hir => descriptor.required_hir(),
                SectionLocation::Mir => descriptor.required_mir(),
                SectionLocation::Lir => descriptor.required_lir(),
                SectionLocation::Manifest => panic!("expected a metadata capability"),
            };
            for view in [ArtifactProfileView::Compile, ArtifactProfileView::Link] {
                for purpose in [MemberPurposeSet::NONE, MemberPurposeSet::COMPILE_AND_LINK] {
                    let mut sections = inventory
                        .iter()
                        .map(|capability| {
                            (
                                capability.clone(),
                                CapabilityContractRegistry::contract(capability)
                                    .unwrap()
                                    .required_for(),
                            )
                        })
                        .collect::<Vec<_>>();
                    sections.push((old.clone(), purpose));
                    assert!(
                        matches!(super::super::validate_inventory(view, contract.location(), inventory, &sections, |s| &s.0, |s| s.1), Err(ArtifactProfileInventoryError::ConflictingCapabilityVersion { required, actual, .. }) if *required == current && *actual == old)
                    );
                }
            }
        }
    }
}

#[test]
fn hir_foundation_v10_preserves_annotation_description_owners() {
    assert_retired_version(
        hir_identity_foundation_capability(),
        10,
        MemberPurposeSet::COMPILE_AND_LINK,
        &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC],
    );
}
