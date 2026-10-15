use scoop_wire::encode;

use super::*;

mod layout;
mod strong_version;

#[test]
fn capability_registry_has_the_fixed_location_purpose_and_sink_matrix() {
    let code_runtime_link = FingerprintSinkSet::CODE
        .union(FingerprintSinkSet::RUNTIME_IMAGE)
        .union(FingerprintSinkSet::LINK_VALIDATION_ONLY);
    let code_runtime = FingerprintSinkSet::CODE.union(FingerprintSinkSet::RUNTIME_IMAGE);
    let code_link = FingerprintSinkSet::CODE.union(FingerprintSinkSet::LINK_VALIDATION_ONLY);
    for (capability, location, purpose, sinks) in [
        (
            hir_identity_foundation_capability(),
            SectionLocation::Hir,
            MemberPurposeSet::COMPILE_AND_LINK,
            FingerprintSinkSet::HIR,
        ),
        (
            mir_identity_foundation_capability(),
            SectionLocation::Mir,
            MemberPurposeSet::COMPILE_AND_LINK,
            code_link,
        ),
        (
            lir_identity_foundation_capability(),
            SectionLocation::Lir,
            MemberPurposeSet::COMPILE_AND_LINK,
            code_link,
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
            MemberPurposeSet::COMPILE_AND_LINK,
            FingerprintSinkSet::LIR,
        ),
        (
            lir_strong_production_capability(),
            SectionLocation::Lir,
            MemberPurposeSet::COMPILE_AND_LINK,
            code_runtime,
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
        ArtifactCapabilityProfile::CROSS_CONE_GENERIC.validate_link_manifest_inventory(&[]),
        Err(ArtifactProfileInventoryError::MissingRequiredCapability {
            view: ArtifactProfileView::Link,
            location: SectionLocation::Manifest,
            capability: manifest_single_cone_production_capability()
        })
    );
    assert!(
        ArtifactCapabilityProfile::CROSS_CONE_GENERIC
            .validate_link_manifest_inventory(std::slice::from_ref(&production))
            .is_ok()
    );
    let optional = crate::ManifestSection::new(
        CapabilityId::new("org.scoop-lang.test", "optional", 1).unwrap(),
        MemberPurposeSet::NONE,
        Vec::new(),
    )
    .unwrap();
    assert!(
        ArtifactCapabilityProfile::CROSS_CONE_GENERIC
            .validate_link_manifest_inventory(&[production, optional])
            .is_ok()
    );
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
    let decoded = crate::DecodedMetadataEnvelope::decode(&bytes, location).unwrap();
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
