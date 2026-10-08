use super::*;

#[test]
fn source_interface_v63_retains_equality_protocol() {
    assert_retired_version(
        hir_cross_cone_interface_capability(),
        63,
        MemberPurposeSet::COMPILE,
        &[
            ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        ],
    );
}

#[test]
fn type_semantics_v23_retains_root_representation() {
    assert_retired_version(
        hir_cross_cone_type_semantics_capability(),
        23,
        MemberPurposeSet::COMPILE,
        &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC],
    );
}

#[test]
fn mir_type_bridge_v16_retains_root_representation() {
    assert_retired_version(
        mir_cross_cone_type_bridge_capability(),
        16,
        MemberPurposeSet::COMPILE,
        &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC],
    );
}

#[test]
fn mir_foundation_v5_retains_callback_snapshot_abi() {
    assert_retired_version(
        mir_identity_foundation_capability(),
        5,
        MemberPurposeSet::COMPILE_AND_LINK,
        &[
            ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        ],
    );
}

#[test]
fn lir_layout_abi_v11_retains_root_references() {
    assert_retired_version(
        lir_cross_cone_layout_abi_capability(),
        11,
        MemberPurposeSet::COMPILE_AND_LINK,
        &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC],
    );
}

#[test]
fn compiler_protocol_v13_retains_equality_protocol() {
    assert_retired_version(
        hir_core_bootstrap_interface_capability(),
        13,
        MemberPurposeSet::COMPILE,
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
fn hir_foundation_v8_retires_structural_encoding_callables() {
    assert_retired_version(
        hir_identity_foundation_capability(),
        8,
        MemberPurposeSet::COMPILE_AND_LINK,
        &[
            ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        ],
    );
}
