use super::*;

#[test]
fn production_manifest_v9_uses_consumed_fields_in_every_profile() {
    let current = manifest_single_cone_production_capability();
    assert_eq!(current.major_version(), 9);
    let old = CapabilityId::new(current.namespace(), current.name(), 8).unwrap();
    assert!(CapabilityContractRegistry::contract(&old).is_none());
    let profile = ArtifactCapabilityProfile::CROSS_CONE_GENERIC;

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

#[test]
fn shared_provider_references_reject_retired_and_cross_profile_versions_in_every_view() {
    for (current, profiles, rejected) in [
        (
            lir_identity_foundation_capability(),
            &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC][..],
            &[1, 2, 3, 4, 5, 6, 7][..],
        ),
        (
            lir_link_identity_closure_capability(),
            &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC][..],
            &[
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
            ][..],
        ),
        (
            lir_cone_production_capability(),
            &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC][..],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15][..],
        ),
        (
            lir_cross_cone_layout_abi_capability(),
            &[ArtifactCapabilityProfile::CROSS_CONE_GENERIC][..],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9][..],
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

#[test]
fn generated_entry_formats_require_the_updated_hir_and_mir_sections() {
    for (current, version) in [
        (hir_identity_foundation_capability(), 10),
        (mir_identity_foundation_capability(), 8),
        (hir_core_bootstrap_interface_capability(), 17),
        (mir_core_bootstrap_bridge_capability(), 2),
    ] {
        assert_eq!(current.major_version(), version);
        assert!(CapabilityContractRegistry::contract(&current).is_some());
        let old = CapabilityId::new(current.namespace(), current.name(), version - 1).unwrap();
        assert!(CapabilityContractRegistry::contract(&old).is_none());
        let descriptor = ArtifactCapabilityProfile::CROSS_CONE_GENERIC.descriptor();
        assert!(
            descriptor.required_hir().contains(&current)
                || descriptor.required_mir().contains(&current)
        );
    }
}
