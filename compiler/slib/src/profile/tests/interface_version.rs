use super::*;

#[test]
fn source_interface_v19_requires_original_source_call_receivers() {
    assert_retired_version(
        hir_cross_cone_interface_capability(),
        19,
        &[
            ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_LAYOUT_STRONG,
        ],
    );
}

#[test]
fn type_semantics_v4_requires_default_call_receiver_signatures() {
    assert_retired_version(
        hir_cross_cone_type_semantics_capability(),
        4,
        &[ArtifactCapabilityProfile::CROSS_CONE_LAYOUT_STRONG],
    );
}

#[test]
fn compiler_protocol_v3_rejects_both_retired_core_qualification_formats_in_all_views() {
    assert_retired_version(
        hir_core_bootstrap_interface_capability(),
        3,
        &[
            ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_LAYOUT_STRONG,
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
    assert_eq!(contract.location(), SectionLocation::Hir);
    assert_eq!(contract.required_for(), MemberPurposeSet::COMPILE);
    assert_eq!(contract.sinks(), FingerprintSinkSet::HIR);
    for retired in 1..major {
        let old = CapabilityId::new(current.namespace(), current.name(), retired).unwrap();
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
}

#[test]
fn hir_foundation_v3_rejects_retired_native_witnesses_in_every_profile_and_view() {
    assert_retired_version(
        hir_identity_foundation_capability(),
        3,
        &[
            ArtifactCapabilityProfile::IDENTITY_FOUNDATION,
            ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
            ArtifactCapabilityProfile::CROSS_CONE_LAYOUT_STRONG,
        ],
    );
}
