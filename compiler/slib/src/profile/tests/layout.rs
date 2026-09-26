use super::*;

#[test]
fn layout_profile_has_a_fixed_descriptor_and_fingerprint() {
    let profile = ArtifactCapabilityProfile::CROSS_CONE_LAYOUT_STRONG;
    let descriptor = profile.descriptor();
    assert_eq!(
        hex(&encode(&descriptor).unwrap()),
        include_str!("layout_profile.hex").trim()
    );
    assert_eq!(
        profile.fingerprint().unwrap().to_string(),
        "563e99b1d4e352c0c696baf88e37b5b778eb06ff129b555c1ce1190200be3076"
    );
    assert_eq!(
        ArtifactCapabilityProfile::from_id(descriptor.id()),
        Some(profile)
    );
    assert_eq!(
        descriptor.validation_policy(),
        ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG
            .descriptor()
            .validation_policy()
    );
    assert_eq!(
        descriptor.validation_policy().odr(),
        OdrValidationPolicy::RejectAll
    );
    assert!(
        descriptor
            .required_lir()
            .contains(&lir_strong_production_v2_capability())
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
            MemberPurposeSet::COMPILE,
            FingerprintSinkSet::LIR,
        ),
        (
            lir_cross_cone_layout_link_closure_capability(),
            SectionLocation::Lir,
            MemberPurposeSet::LINK,
            FingerprintSinkSet::CODE.union(FingerprintSinkSet::LINK_VALIDATION_ONLY),
        ),
        (
            lir_strong_production_v2_capability(),
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
    let profile = ArtifactCapabilityProfile::CROSS_CONE_LAYOUT_STRONG;
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
fn layout_profile_rejects_both_strong_versions_even_when_v1_is_optional() {
    let descriptor = ArtifactCapabilityProfile::CROSS_CONE_LAYOUT_STRONG.descriptor();
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
            sections.push((lir_strong_production_capability(), purpose));
            assert!(matches!(validate_inventory(view, SectionLocation::Lir,
                descriptor.required_lir(), &sections, |section| &section.0, |section| section.1),
                Err(ArtifactProfileInventoryError::ConflictingCapabilityVersion { required, actual, .. })
                    if *required == lir_strong_production_v2_capability() && *actual == lir_strong_production_capability()));
        }
    }
}

#[test]
fn old_profiles_do_not_gain_new_layout_authority() {
    let descriptor = ArtifactCapabilityProfile::CROSS_CONE_LAYOUT_STRONG.descriptor();
    for profile in [
        ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
        ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
    ] {
        for view in [ArtifactProfileView::Compile, ArtifactProfileView::Link] {
            assert!(matches!(
                validate_metadata_inventory(
                    profile,
                    view,
                    crate::MetadataLocation::Lir,
                    descriptor.required_lir()
                ),
                Err(ArtifactProfileInventoryError::UnsupportedRequiredCapability { .. })
            ));
        }
    }
}
