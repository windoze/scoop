use super::*;

#[test]
fn shared_initialization_abi_retires_both_core_branch_formats_in_every_view() {
    for (current, profiles, rejected) in [
        (
            lir_strong_production_capability(),
            &[
                ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
                ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
            ][..],
            [1, 2, 4],
        ),
        (
            lir_strong_production_v2_capability(),
            &[ArtifactCapabilityProfile::CROSS_CONE_LAYOUT_STRONG][..],
            [1, 2, 3],
        ),
    ] {
        for version in rejected {
            let old = CapabilityId::new(current.namespace(), current.name(), version).unwrap();
            if version <= 2 {
                assert!(CapabilityContractRegistry::contract(&old).is_none());
            }
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
