use super::*;

#[test]
fn source_interface_v35_requires_template_evaluation_locations() {
    assert_retired_version(
        hir_cross_cone_interface_capability(),
        35,
        &[
            ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        ],
    );
}

#[test]
fn type_semantics_v10_retires_duplicate_slot_domains() {
    assert_retired_version(
        hir_cross_cone_type_semantics_capability(),
        10,
        &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC],
    );
}

#[test]
fn mir_type_bridge_v4_separates_interface_contracts_from_machine_targets() {
    assert_retired_version(
        mir_cross_cone_type_bridge_capability(),
        4,
        &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC],
    );
}

#[test]
fn lir_layout_abi_v5_requires_application_callable_definitions() {
    assert_retired_version(
        lir_cross_cone_layout_abi_capability(),
        5,
        &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC],
    );
}

#[test]
fn compiler_protocol_v4_rejects_retired_protocol_wrappers_in_all_views() {
    assert_retired_version(
        hir_core_bootstrap_interface_capability(),
        4,
        &[
            ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        ],
    );
}

fn assert_retired_version(
    current: CapabilityId,
    major: u32,
    profiles: &[ArtifactCapabilityProfile],
) {
    let contract = CapabilityContractRegistry::contract(&current).unwrap();
    assert_eq!(current.major_version(), major);
    assert_eq!(contract.required_for(), MemberPurposeSet::COMPILE);
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
fn hir_foundation_v3_rejects_retired_native_witnesses_in_every_profile_and_view() {
    assert_retired_version(
        hir_identity_foundation_capability(),
        3,
        &[
            ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        ],
    );
}
