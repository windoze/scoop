use super::*;

#[test]
fn source_interface_v2_has_required_hir_semantics_and_rejects_retired_v1() {
    assert_retired_version(
        hir_cross_cone_interface_capability(),
        &[
            ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_LAYOUT_STRONG,
        ],
    );
}

#[test]
fn compiler_protocol_v2_retires_the_total_operation_table_capability_in_all_views() {
    assert_retired_version(
        hir_core_bootstrap_interface_capability(),
        &[
            ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_LAYOUT_STRONG,
        ],
    );
}

fn assert_retired_version(current: CapabilityId, profiles: &[ArtifactCapabilityProfile]) {
    let old = CapabilityId::new(current.namespace(), current.name(), 1).unwrap();
    let contract = CapabilityContractRegistry::contract(&current).unwrap();
    assert_eq!(current.major_version(), 2);
    assert_eq!(contract.location(), SectionLocation::Hir);
    assert_eq!(contract.required_for(), MemberPurposeSet::COMPILE);
    assert_eq!(contract.sinks(), FingerprintSinkSet::HIR);
    assert!(CapabilityContractRegistry::contract(&old).is_none());
    for profile in profiles {
        let descriptor = profile.descriptor();
        for view in [ArtifactProfileView::Compile, ArtifactProfileView::Link] {
            for purpose in [MemberPurposeSet::NONE, MemberPurposeSet::COMPILE_AND_LINK] {
                let mut sections = descriptor
                    .required_hir()
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
                    matches!(super::super::validate_inventory(view, SectionLocation::Hir, descriptor.required_hir(), &sections, |s| &s.0, |s| s.1), Err(ArtifactProfileInventoryError::ConflictingCapabilityVersion { required, actual, .. }) if *required == current && *actual == old)
                );
            }
        }
    }
}
