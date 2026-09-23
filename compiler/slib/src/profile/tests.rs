use scoop_wire::{DecodeLimits, encode};

use super::*;

mod layout;
mod strong_version;

#[test]
fn identity_foundation_profile_has_the_fixed_descriptor_and_fingerprint() {
    let profile = ArtifactCapabilityProfile::IDENTITY_FOUNDATION;
    let descriptor = profile.descriptor();
    assert_eq!(
        hex(&encode(&descriptor).unwrap()),
        "a901a301781b6f72672e73636f6f702d6c616e672e736c69622d70726f66696c6502736964656e746974792d666f756e646174696f6e030102800381a301726f72672e73636f6f702d6c616e672e68697202736964656e746974792d666f756e646174696f6e03030481a301726f72672e73636f6f702d6c616e672e6d697202736964656e746974792d666f756e646174696f6e03010581a301726f72672e73636f6f702d6c616e672e6c697202736964656e746974792d666f756e646174696f6e030106010701080109a40101020103010401"
    );
    assert_eq!(
        profile.fingerprint().unwrap().to_string(),
        "a2172217523ca6415bf16fa191ec9af7bfd9f10a64970935713ab49a2cecb59d"
    );
    assert_eq!(
        descriptor.publication_class(),
        PublicationClass::FoundationOnly
    );
    assert_eq!(
        descriptor.validation_policy().link_proof(),
        LinkProofPolicy::Forbidden
    );
}

#[test]
fn single_cone_strong_profile_has_the_fixed_descriptor_and_fingerprint() {
    let profile = ArtifactCapabilityProfile::SINGLE_CONE_STRONG;
    let descriptor = profile.descriptor();
    assert_eq!(
        hex(&encode(&descriptor).unwrap()),
        "a901a301781b6f72672e73636f6f702d6c616e672e736c69622d70726f66696c65027273696e676c652d636f6e652d7374726f6e6703010281a301776f72672e73636f6f702d6c616e672e6d616e6966657374027673696e676c652d636f6e652d70726f64756374696f6e03010382a301726f72672e73636f6f702d6c616e672e686972027818636f72652d626f6f7473747261702d696e746572666163650303a301726f72672e73636f6f702d6c616e672e68697202736964656e746974792d666f756e646174696f6e03030482a301726f72672e73636f6f702d6c616e672e6d69720275636f72652d626f6f7473747261702d6272696467650301a301726f72672e73636f6f702d6c616e672e6d697202736964656e746974792d666f756e646174696f6e03010583a301726f72672e73636f6f702d6c616e672e6c697202736964656e746974792d666f756e646174696f6e0301a301726f72672e73636f6f702d6c616e672e6c697202756c696e6b2d6964656e746974792d636c6f737572650302a301726f72672e73636f6f702d6c616e672e6c697202717374726f6e672d70726f64756374696f6e030506020702080209a40102020103010402"
    );
    assert_eq!(
        profile.fingerprint().unwrap().to_string(),
        "e619b772b4535471ae3550d1c5abf69e758ffc8287a1db30ece8b02a1b6df981"
    );
    assert_eq!(
        ArtifactCapabilityProfile::from_id(descriptor.id()),
        Some(profile)
    );

    assert_eq!(
        descriptor.required_manifest(),
        &[manifest_single_cone_production_capability()]
    );
    assert_eq!(
        descriptor.required_hir(),
        &[
            hir_core_bootstrap_interface_capability(),
            hir_identity_foundation_capability(),
        ]
    );
    assert_eq!(
        descriptor.required_mir(),
        &[
            mir_core_bootstrap_bridge_capability(),
            mir_identity_foundation_capability(),
        ]
    );
    assert_eq!(
        descriptor.required_lir(),
        &[
            lir_identity_foundation_capability(),
            lir_link_identity_closure_capability(),
            lir_strong_production_capability(),
        ]
    );
    assert_eq!(
        descriptor.code_requirement(),
        FingerprintAvailabilityRequirement::MustBeAvailable
    );
    assert_eq!(
        descriptor.runtime_requirement(),
        FingerprintAvailabilityRequirement::MustBeAvailable
    );
    assert_eq!(
        descriptor.publication_class(),
        PublicationClass::Publishable
    );
    assert_eq!(
        descriptor.validation_policy().odr(),
        OdrValidationPolicy::RejectAll
    );
    assert_eq!(
        descriptor.validation_policy().link_proof(),
        LinkProofPolicy::Required
    );
}

#[test]
fn cross_cone_semantics_strong_profile_has_the_fixed_descriptor_and_fingerprint() {
    let profile = ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG;
    let descriptor = profile.descriptor();
    assert_eq!(
        hex(&encode(&descriptor).unwrap()),
        "a901a301781b6f72672e73636f6f702d6c616e672e736c69622d70726f66696c6502781b63726f73732d636f6e652d73656d616e746963732d7374726f6e6703010281a301776f72672e73636f6f702d6c616e672e6d616e6966657374027673696e676c652d636f6e652d70726f64756374696f6e03010383a301726f72672e73636f6f702d6c616e672e686972027818636f72652d626f6f7473747261702d696e746572666163650303a301726f72672e73636f6f702d6c616e672e686972027463726f73732d636f6e652d696e746572666163650304a301726f72672e73636f6f702d6c616e672e68697202736964656e746974792d666f756e646174696f6e03030483a301726f72672e73636f6f702d6c616e672e6d69720275636f72652d626f6f7473747261702d6272696467650301a301726f72672e73636f6f702d6c616e672e6d697202781c63726f73732d636f6e652d706172616d2d667265652d6272696467650301a301726f72672e73636f6f702d6c616e672e6d697202736964656e746974792d666f756e646174696f6e03010585a301726f72672e73636f6f702d6c616e672e6c6972027763726f73732d636f6e652d6c696e6b2d636c6f737572650301a301726f72672e73636f6f702d6c616e672e6c697202781c63726f73732d636f6e652d706172616d2d667265652d6272696467650301a301726f72672e73636f6f702d6c616e672e6c697202736964656e746974792d666f756e646174696f6e0301a301726f72672e73636f6f702d6c616e672e6c697202756c696e6b2d6964656e746974792d636c6f737572650302a301726f72672e73636f6f702d6c616e672e6c697202717374726f6e672d70726f64756374696f6e030506020702080209a40102020103010402"
    );
    assert_eq!(
        profile.fingerprint().unwrap().to_string(),
        "51a7cd21a9887c2f73bff35bfc70cf830ec092bfc33f0af48bb9395ef5532970"
    );

    assert_eq!(
        ArtifactCapabilityProfile::from_id(descriptor.id()),
        Some(profile)
    );
    assert_eq!(
        descriptor.required_manifest(),
        &[manifest_single_cone_production_capability()]
    );
    assert_eq!(
        descriptor.required_hir(),
        &[
            hir_core_bootstrap_interface_capability(),
            hir_cross_cone_interface_capability(),
            hir_identity_foundation_capability(),
        ]
    );
    assert_eq!(
        descriptor.required_mir(),
        &[
            mir_core_bootstrap_bridge_capability(),
            mir_cross_cone_param_free_bridge_capability(),
            mir_identity_foundation_capability(),
        ]
    );
    assert_eq!(
        descriptor.required_lir(),
        &[
            lir_cross_cone_link_closure_capability(),
            lir_cross_cone_param_free_bridge_capability(),
            lir_identity_foundation_capability(),
            lir_link_identity_closure_capability(),
            lir_strong_production_capability(),
        ]
    );
    assert_eq!(
        descriptor.code_requirement(),
        FingerprintAvailabilityRequirement::MustBeAvailable
    );
    assert_eq!(
        descriptor.runtime_requirement(),
        FingerprintAvailabilityRequirement::MustBeAvailable
    );
    assert_eq!(
        descriptor.publication_class(),
        PublicationClass::Publishable
    );
    assert_eq!(
        descriptor.validation_policy(),
        ArtifactCapabilityProfile::SINGLE_CONE_STRONG
            .descriptor()
            .validation_policy()
    );
}

#[test]
fn capability_registry_has_the_fixed_location_purpose_and_sink_matrix() {
    let code_runtime_link = FingerprintSinkSet::CODE
        .union(FingerprintSinkSet::RUNTIME_IMAGE)
        .union(FingerprintSinkSet::LINK_VALIDATION_ONLY);
    let lir_code_runtime = FingerprintSinkSet::LIR
        .union(FingerprintSinkSet::CODE)
        .union(FingerprintSinkSet::RUNTIME_IMAGE);
    let code_link = FingerprintSinkSet::CODE.union(FingerprintSinkSet::LINK_VALIDATION_ONLY);
    for (capability, location, purpose, sinks) in [
        (
            hir_identity_foundation_capability(),
            SectionLocation::Hir,
            MemberPurposeSet::COMPILE,
            FingerprintSinkSet::HIR,
        ),
        (
            mir_identity_foundation_capability(),
            SectionLocation::Mir,
            MemberPurposeSet::COMPILE,
            FingerprintSinkSet::MIR,
        ),
        (
            lir_identity_foundation_capability(),
            SectionLocation::Lir,
            MemberPurposeSet::COMPILE,
            FingerprintSinkSet::LIR,
        ),
        (
            manifest_single_cone_production_capability(),
            SectionLocation::Manifest,
            MemberPurposeSet::LINK,
            code_runtime_link,
        ),
        (
            hir_core_bootstrap_interface_capability(),
            SectionLocation::Hir,
            MemberPurposeSet::COMPILE,
            FingerprintSinkSet::HIR,
        ),
        (
            hir_cross_cone_interface_capability(),
            SectionLocation::Hir,
            MemberPurposeSet::COMPILE,
            FingerprintSinkSet::HIR,
        ),
        (
            mir_core_bootstrap_bridge_capability(),
            SectionLocation::Mir,
            MemberPurposeSet::COMPILE,
            FingerprintSinkSet::MIR,
        ),
        (
            mir_cross_cone_param_free_bridge_capability(),
            SectionLocation::Mir,
            MemberPurposeSet::COMPILE,
            FingerprintSinkSet::MIR,
        ),
        (
            lir_cross_cone_param_free_bridge_capability(),
            SectionLocation::Lir,
            MemberPurposeSet::COMPILE,
            FingerprintSinkSet::LIR,
        ),
        (
            lir_cross_cone_link_closure_capability(),
            SectionLocation::Lir,
            MemberPurposeSet::LINK,
            code_link,
        ),
        (
            lir_strong_production_capability(),
            SectionLocation::Lir,
            MemberPurposeSet::COMPILE_AND_LINK,
            lir_code_runtime,
        ),
        (
            lir_link_identity_closure_capability(),
            SectionLocation::Lir,
            MemberPurposeSet::LINK,
            FingerprintSinkSet::LINK_VALIDATION_ONLY,
        ),
    ] {
        let contract = CapabilityContractRegistry::contract(&capability).unwrap();
        assert_eq!(contract.capability(), &capability);
        assert_eq!(contract.location(), location);
        assert_eq!(contract.required_for(), purpose);
        assert_eq!(contract.sinks(), sinks);
    }

    assert!(
        CapabilityContractRegistry::contract(
            &CapabilityId::new("org.scoop-lang.test", "unknown", 1).unwrap()
        )
        .is_none()
    );
}

#[test]
fn manifest_inventory_is_closed_by_artifact_profile() {
    let production = crate::ManifestSection::new(
        manifest_single_cone_production_capability(),
        MemberPurposeSet::LINK,
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        ArtifactCapabilityProfile::SINGLE_CONE_STRONG.validate_link_manifest_inventory(&[]),
        Err(ArtifactProfileInventoryError::MissingRequiredCapability {
            view: ArtifactProfileView::Link,
            location: SectionLocation::Manifest,
            capability: manifest_single_cone_production_capability()
        })
    );
    assert!(
        ArtifactCapabilityProfile::SINGLE_CONE_STRONG
            .validate_link_manifest_inventory(std::slice::from_ref(&production))
            .is_ok()
    );
    assert_eq!(
        ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG
            .validate_link_manifest_inventory(&[]),
        Err(ArtifactProfileInventoryError::MissingRequiredCapability {
            view: ArtifactProfileView::Link,
            location: SectionLocation::Manifest,
            capability: manifest_single_cone_production_capability()
        })
    );
    assert!(
        ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG
            .validate_link_manifest_inventory(std::slice::from_ref(&production))
            .is_ok()
    );
    assert!(matches!(
        ArtifactCapabilityProfile::IDENTITY_FOUNDATION
            .validate_link_manifest_inventory(std::slice::from_ref(&production)),
        Err(
            ArtifactProfileInventoryError::UnsupportedRequiredCapability {
                view: ArtifactProfileView::Link,
                location: SectionLocation::Manifest,
                index: 0,
                ..
            }
        )
    ));

    let optional = crate::ManifestSection::new(
        CapabilityId::new("org.scoop-lang.test", "optional", 1).unwrap(),
        MemberPurposeSet::NONE,
        Vec::new(),
    )
    .unwrap();
    assert!(
        ArtifactCapabilityProfile::IDENTITY_FOUNDATION
            .validate_link_manifest_inventory(&[optional])
            .is_ok()
    );
}

#[test]
fn cross_cone_metadata_inventory_requires_each_new_capability_in_its_view() {
    let profile = ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG;
    let descriptor = profile.descriptor();
    assert!(
        validate_metadata_inventory(
            profile,
            ArtifactProfileView::Compile,
            crate::MetadataLocation::Hir,
            descriptor.required_hir(),
        )
        .is_ok()
    );
    assert!(
        validate_metadata_inventory(
            profile,
            ArtifactProfileView::Compile,
            crate::MetadataLocation::Mir,
            descriptor.required_mir(),
        )
        .is_ok()
    );
    assert!(
        validate_metadata_inventory(
            profile,
            ArtifactProfileView::Compile,
            crate::MetadataLocation::Lir,
            descriptor.required_lir(),
        )
        .is_ok()
    );
    assert!(
        validate_metadata_inventory(
            profile,
            ArtifactProfileView::Link,
            crate::MetadataLocation::Lir,
            descriptor.required_lir(),
        )
        .is_ok()
    );

    for (view, location, capability, inventory) in [
        (
            ArtifactProfileView::Compile,
            crate::MetadataLocation::Hir,
            hir_cross_cone_interface_capability(),
            descriptor.required_hir(),
        ),
        (
            ArtifactProfileView::Compile,
            crate::MetadataLocation::Mir,
            mir_cross_cone_param_free_bridge_capability(),
            descriptor.required_mir(),
        ),
        (
            ArtifactProfileView::Compile,
            crate::MetadataLocation::Lir,
            lir_cross_cone_param_free_bridge_capability(),
            descriptor.required_lir(),
        ),
        (
            ArtifactProfileView::Link,
            crate::MetadataLocation::Lir,
            lir_cross_cone_link_closure_capability(),
            descriptor.required_lir(),
        ),
    ] {
        let incomplete = inventory
            .iter()
            .filter(|candidate| *candidate != &capability)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(
            validate_metadata_inventory(profile, view, location, &incomplete),
            Err(ArtifactProfileInventoryError::MissingRequiredCapability {
                view,
                location: match location {
                    crate::MetadataLocation::Hir => SectionLocation::Hir,
                    crate::MetadataLocation::Mir => SectionLocation::Mir,
                    crate::MetadataLocation::Lir => SectionLocation::Lir,
                },
                capability,
            })
        );
    }
}

fn validate_metadata_inventory(
    profile: ArtifactCapabilityProfile,
    view: ArtifactProfileView,
    location: crate::MetadataLocation,
    capabilities: &[CapabilityId],
) -> Result<(), ArtifactProfileInventoryError> {
    let sections = capabilities
        .iter()
        .map(|capability| {
            let contract = CapabilityContractRegistry::contract(capability).unwrap();
            crate::MetadataSection::new(
                location,
                capability.clone(),
                contract.required_for(),
                Vec::new(),
            )
            .unwrap()
        })
        .collect();
    let envelope = crate::MetadataEnvelope::new(location, sections).unwrap();
    let bytes = encode(&envelope).unwrap();
    let decoded =
        crate::DecodedMetadataEnvelope::decode(&bytes, location, DecodeLimits::default()).unwrap();
    match view {
        ArtifactProfileView::Compile => {
            profile.validate_compile_metadata_inventory(location, decoded.sections())
        }
        ArtifactProfileView::Link => {
            profile.validate_link_metadata_inventory(location, decoded.sections())
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod interface_version;
