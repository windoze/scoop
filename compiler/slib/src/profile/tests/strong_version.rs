use super::*;

#[test]
fn production_manifest_v5_requires_the_physical_odr_directory_in_every_profile() {
    let current = manifest_single_cone_production_capability();
    assert_eq!(current.major_version(), 5);
    let old = CapabilityId::new(current.namespace(), current.name(), 1).unwrap();
    assert!(CapabilityContractRegistry::contract(&old).is_none());
    for profile in [
        ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
        ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
        ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
    ] {
        let descriptor = profile.descriptor();
        assert_eq!(
            descriptor.required_manifest(),
            std::slice::from_ref(&current)
        );
        for view in [ArtifactProfileView::Compile, ArtifactProfileView::Link] {
            for purpose in [MemberPurposeSet::NONE, MemberPurposeSet::LINK] {
                let sections = [
                    (current.clone(), MemberPurposeSet::LINK),
                    (old.clone(), purpose),
                ];
                assert!(matches!(
                    super::super::validate_inventory(
                        view, SectionLocation::Manifest, descriptor.required_manifest(), &sections,
                        |section| &section.0, |section| section.1,
                    ),
                    Err(ArtifactProfileInventoryError::ConflictingCapabilityVersion { required, actual, .. })
                        if *required == current && *actual == old
                ));
            }
        }
    }
}

#[test]
fn shared_provider_references_reject_retired_and_cross_profile_versions_in_every_view() {
    for (current, profiles, rejected) in [
        (
            lir_identity_foundation_capability(),
            &[
                ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
                ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
                ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
            ][..],
            &[1, 2, 3, 4, 5, 6][..],
        ),
        (
            lir_link_identity_closure_capability(),
            &[
                ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
                ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
                ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
            ][..],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14][..],
        ),
        (
            lir_strong_production_capability(),
            &[
                ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
                ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
            ][..],
            &[
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
            ][..],
        ),
        (
            lir_cone_production_capability(),
            &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC][..],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9][..],
        ),
        (
            lir_cross_cone_layout_abi_capability(),
            &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC][..],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9][..],
        ),
        (
            lir_cross_cone_layout_link_closure_capability(),
            &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC][..],
            &[1, 2, 3, 4][..],
        ),
    ] {
        for &version in rejected {
            let old = CapabilityId::new(current.namespace(), current.name(), version).unwrap();
            assert!(CapabilityContractRegistry::contract(&old).is_none());
            for profile in profiles {
                let descriptor = profile.descriptor();
                for view in [ArtifactProfileView::Compile, ArtifactProfileView::Link] {
                    for purpose in [MemberPurposeSet::NONE, MemberPurposeSet::COMPILE_AND_LINK] {
                        let mut sections = descriptor
                            .required_lir()
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
                        assert!(matches!(
                            super::super::validate_inventory(
                                view, SectionLocation::Lir, descriptor.required_lir(), &sections,
                                |section| &section.0, |section| section.1,
                            ),
                            Err(ArtifactProfileInventoryError::ConflictingCapabilityVersion { required, actual, .. })
                                if *required == current && *actual == old
                        ));
                    }
                }
            }
        }
    }
}
