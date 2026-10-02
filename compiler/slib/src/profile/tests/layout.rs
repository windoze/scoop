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
        "a591e4fa2c4360349486b7c7afb10617eda1747b1484a13933549e788e2b955f"
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
            lir_cross_cone_layout_link_closure_capability(),
            SectionLocation::Lir,
            MemberPurposeSet::LINK,
            FingerprintSinkSet::CODE.union(FingerprintSinkSet::LINK_VALIDATION_ONLY),
        ),
        (
            lir_cone_production_capability(),
            SectionLocation::Lir,
            MemberPurposeSet::COMPILE_AND_LINK,
            FingerprintSinkSet::LIR
                .union(FingerprintSinkSet::CODE)
                .union(FingerprintSinkSet::RUNTIME_IMAGE),
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
    for version in [13, 14, 15, 16] {
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

#[test]
fn legacy_strong_profiles_reject_generic_sections() {
    let descriptor = ArtifactCapabilityProfile::CROSS_CONE_GENERIC.descriptor();
    for profile in [
        ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
        ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
    ] {
        let legacy = profile.descriptor();
        for view in [ArtifactProfileView::Compile, ArtifactProfileView::Link] {
            assert!(matches!(
                validate_metadata_inventory(
                    profile,
                    view,
                    crate::MetadataLocation::Lir,
                    descriptor.required_lir()
                ),
                Err(ArtifactProfileInventoryError::ConflictingCapabilityVersion { required, actual, .. })
                    if *required == lir_strong_production_capability() && *actual == lir_cone_production_capability()
            ));
            for purpose in [MemberPurposeSet::NONE, MemberPurposeSet::COMPILE_AND_LINK] {
                let mut sections = legacy
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
                sections.push((lir_cone_production_capability(), purpose));
                assert!(matches!(validate_inventory(view, SectionLocation::Lir,
                    legacy.required_lir(), &sections, |section| &section.0, |section| section.1),
                    Err(ArtifactProfileInventoryError::ConflictingCapabilityVersion { required, actual, .. })
                        if *required == lir_strong_production_capability() && *actual == lir_cone_production_capability()));
            }
        }
    }
}
