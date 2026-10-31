use super::*;

#[test]
fn odr_directory_uses_lir_and_the_retired_required_manifest_is_rejected() {
    let current = lir_link_identity_closure_capability();
    assert_eq!(current.major_version(), 21);
    let profile = ArtifactCapabilityProfile::CROSS_CONE_GENERIC;
    let descriptor = profile.descriptor();
    assert!(descriptor.required_manifest().is_empty());
    assert!(descriptor.required_lir().contains(&current));
    let retired =
        CapabilityId::new("org.scoop-lang.manifest", "single-cone-production", 10).unwrap();
    assert!(CapabilityContractRegistry::contract(&retired).is_none());
    let section =
        crate::ManifestSection::new(retired.clone(), MemberPurposeSet::LINK, Vec::new()).unwrap();
    assert!(matches!(
        profile.validate_link_manifest_inventory(&[section]),
        Err(ArtifactProfileInventoryError::UnsupportedRequiredCapability { capability, .. })
            if capability == retired
    ));
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
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
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
