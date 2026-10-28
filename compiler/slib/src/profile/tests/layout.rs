use super::*;

#[test]
fn generic_profile_has_a_fixed_descriptor_and_fingerprint() {
    let profile = ArtifactCapabilityProfile::CROSS_CONE_GENERIC;
    let descriptor = profile.descriptor();
    assert_eq!(
        hex(&encode(&descriptor).unwrap()),
        include_str!("layout_profile.hex").trim()
    );
    assert_eq!(
        profile.fingerprint().unwrap().to_string(),
        "459f91ab20c66de65f0ddcc7923327670789608009c75c915cac726174e66db8"
    );
    assert_eq!(
        ArtifactCapabilityProfile::from_id(descriptor.id()),
        Some(profile)
    );
    assert!(
        descriptor
            .required_lir()
            .contains(&lir_cone_production_capability())
    );
    assert!(
        !descriptor
            .required_lir()
            .contains(&lir_strong_production_capability())
    );
}

#[test]
fn layout_sections_have_distinct_semantic_and_physical_purposes() {
    for (capability, location, purpose, sinks) in [
        (
            hir_cross_cone_type_semantics_capability(),
            SectionLocation::Hir,
            MemberPurposeSet::COMPILE,
            FingerprintSinkSet::HIR,
        ),
        (
            mir_cross_cone_type_bridge_capability(),
            SectionLocation::Mir,
            MemberPurposeSet::COMPILE,
            FingerprintSinkSet::MIR,
        ),
        (
            lir_cross_cone_layout_abi_capability(),
            SectionLocation::Lir,
            MemberPurposeSet::COMPILE_AND_LINK,
            FingerprintSinkSet::LIR,
        ),
        (
            lir_cone_production_capability(),
            SectionLocation::Lir,
            MemberPurposeSet::COMPILE_AND_LINK,
            FingerprintSinkSet::CODE.union(FingerprintSinkSet::RUNTIME_IMAGE),
        ),
    ] {
        let contract = CapabilityContractRegistry::contract(&capability).unwrap();
        assert_eq!(contract.location(), location);
        assert_eq!(contract.required_for(), purpose);
        assert_eq!(contract.sinks(), sinks);
    }
}

#[test]
fn each_layout_capability_is_required_before_payload_validation() {
    let profile = ArtifactCapabilityProfile::CROSS_CONE_GENERIC;
    let descriptor = profile.descriptor();
    for (location, inventory) in [
        (crate::MetadataLocation::Hir, descriptor.required_hir()),
        (crate::MetadataLocation::Mir, descriptor.required_mir()),
        (crate::MetadataLocation::Lir, descriptor.required_lir()),
    ] {
        for view in [ArtifactProfileView::Compile, ArtifactProfileView::Link] {
            validate_metadata_inventory(profile, view, location, inventory).unwrap();
            for capability in inventory {
                let contract = CapabilityContractRegistry::contract(capability).unwrap();
                if !contract.required_for().contains(view.member_purpose()) {
                    continue;
                }
                let incomplete = inventory
                    .iter()
                    .filter(|item| *item != capability)
                    .cloned()
                    .collect::<Vec<_>>();
                assert!(
                    matches!(validate_metadata_inventory(profile, view, location, &incomplete),
                    Err(ArtifactProfileInventoryError::MissingRequiredCapability { capability: missing, .. })
                        if missing == *capability)
                );
            }
        }
    }
}

#[test]
fn generic_profile_rejects_legacy_strong_production_even_when_optional() {
    let descriptor = ArtifactCapabilityProfile::CROSS_CONE_GENERIC.descriptor();
    for version in [13, 14, 15, 16, 17, 18, 19] {
        let legacy = CapabilityId::new("org.scoop-lang.lir", "strong-production", version).unwrap();
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
                sections.push((legacy.clone(), purpose));
                assert!(matches!(validate_inventory(view, SectionLocation::Lir,
                    descriptor.required_lir(), &sections, |section| &section.0, |section| section.1),
                    Err(ArtifactProfileInventoryError::ConflictingCapabilityVersion { required, actual, .. })
                        if *required == lir_cone_production_capability() && *actual == legacy));
            }
        }
    }
}
