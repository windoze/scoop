use std::fmt;

use scoop_identity::{ArtifactCapabilityProfileId, CapabilityId};
use scoop_wire::{Encoder, HashError, WireEncode, domain_separated_cbor_hash};

use crate::MemberPurposeSet;

const ARTIFACT_PROFILE_DOMAIN: &str = "scoop-artifact-capability-profile-v1";

mod capabilities;
mod contracts;
mod descriptor;
mod inventory;
#[cfg(test)]
mod tests;

pub use capabilities::*;
pub use contracts::*;
pub use descriptor::*;
use inventory::validate_inventory;
pub use inventory::{ArtifactProfileInventoryError, ArtifactProfileView};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ArtifactCapabilityProfile(ArtifactCapabilityProfileKind);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum ArtifactCapabilityProfileKind {
    IdentityFoundation,
    SingleConeStrong,
    CrossConeSemanticsStrong,
    CrossConeLayoutStrong,
}

impl ArtifactCapabilityProfile {
    pub const IDENTITY_FOUNDATION: Self = Self(ArtifactCapabilityProfileKind::IdentityFoundation);
    pub const SINGLE_CONE_STRONG: Self = Self(ArtifactCapabilityProfileKind::SingleConeStrong);
    pub const CROSS_CONE_SEMANTICS_STRONG: Self =
        Self(ArtifactCapabilityProfileKind::CrossConeSemanticsStrong);
    pub const CROSS_CONE_LAYOUT_STRONG: Self =
        Self(ArtifactCapabilityProfileKind::CrossConeLayoutStrong);

    pub fn id(self) -> ArtifactCapabilityProfileId {
        match self.0 {
            ArtifactCapabilityProfileKind::IdentityFoundation => {
                ArtifactCapabilityProfileId::identity_foundation()
            }
            ArtifactCapabilityProfileKind::SingleConeStrong => {
                ArtifactCapabilityProfileId::single_cone_strong()
            }
            ArtifactCapabilityProfileKind::CrossConeSemanticsStrong => {
                ArtifactCapabilityProfileId::cross_cone_semantics_strong()
            }
            ArtifactCapabilityProfileKind::CrossConeLayoutStrong => {
                ArtifactCapabilityProfileId::cross_cone_layout_strong()
            }
        }
    }

    pub fn from_id(id: &ArtifactCapabilityProfileId) -> Option<Self> {
        if id == &ArtifactCapabilityProfileId::identity_foundation() {
            Some(Self::IDENTITY_FOUNDATION)
        } else if id == &ArtifactCapabilityProfileId::single_cone_strong() {
            Some(Self::SINGLE_CONE_STRONG)
        } else if id == &ArtifactCapabilityProfileId::cross_cone_semantics_strong() {
            Some(Self::CROSS_CONE_SEMANTICS_STRONG)
        } else if id == &ArtifactCapabilityProfileId::cross_cone_layout_strong() {
            Some(Self::CROSS_CONE_LAYOUT_STRONG)
        } else {
            None
        }
    }

    pub fn descriptor(self) -> ArtifactCapabilityProfileDescriptor {
        match self.0 {
            ArtifactCapabilityProfileKind::CrossConeLayoutStrong => {
                let mut descriptor = Self::CROSS_CONE_SEMANTICS_STRONG.descriptor();
                descriptor.id = self.id();
                descriptor
                    .required_hir
                    .push(hir_cross_cone_type_semantics_capability());
                descriptor
                    .required_mir
                    .push(mir_cross_cone_type_bridge_capability());
                descriptor
                    .required_lir
                    .retain(|capability| capability != &lir_strong_production_capability());
                descriptor.required_lir.extend([
                    lir_cross_cone_layout_abi_capability(),
                    lir_cross_cone_layout_link_closure_capability(),
                    lir_strong_production_v2_capability(),
                ]);
                descriptor.required_hir.sort_unstable();
                descriptor.required_mir.sort_unstable();
                descriptor.required_lir.sort_unstable();
                descriptor
            }
            ArtifactCapabilityProfileKind::IdentityFoundation => {
                ArtifactCapabilityProfileDescriptor {
                    id: self.id(),
                    required_manifest: Vec::new(),
                    required_hir: vec![hir_identity_foundation_capability()],
                    required_mir: vec![mir_identity_foundation_capability()],
                    required_lir: vec![lir_identity_foundation_capability()],
                    code_requirement: FingerprintAvailabilityRequirement::MustBeUnavailable,
                    runtime_requirement: FingerprintAvailabilityRequirement::MustBeUnavailable,
                    publication_class: PublicationClass::FoundationOnly,
                    validation_policy: ArtifactValidationPolicy {
                        odr: OdrValidationPolicy::IdentityOnlyNonPublishable,
                        extra_sections:
                            ExtraSectionPolicy::AllowPurposeDisjointOpaqueAndEnvelopeOptional,
                        link_proof: LinkProofPolicy::Forbidden,
                    },
                }
            }
            ArtifactCapabilityProfileKind::SingleConeStrong => {
                ArtifactCapabilityProfileDescriptor {
                    id: self.id(),
                    required_manifest: vec![manifest_single_cone_production_capability()],
                    required_hir: vec![
                        hir_core_bootstrap_interface_capability(),
                        hir_identity_foundation_capability(),
                    ],
                    required_mir: vec![
                        mir_core_bootstrap_bridge_capability(),
                        mir_identity_foundation_capability(),
                    ],
                    required_lir: vec![
                        lir_identity_foundation_capability(),
                        lir_link_identity_closure_capability(),
                        lir_strong_production_capability(),
                    ],
                    code_requirement: FingerprintAvailabilityRequirement::MustBeAvailable,
                    runtime_requirement: FingerprintAvailabilityRequirement::MustBeAvailable,
                    publication_class: PublicationClass::Publishable,
                    validation_policy: ArtifactValidationPolicy {
                        odr: OdrValidationPolicy::RejectAll,
                        extra_sections:
                            ExtraSectionPolicy::AllowPurposeDisjointOpaqueAndEnvelopeOptional,
                        link_proof: LinkProofPolicy::Required,
                    },
                }
            }
            ArtifactCapabilityProfileKind::CrossConeSemanticsStrong => {
                ArtifactCapabilityProfileDescriptor {
                    id: self.id(),
                    required_manifest: vec![manifest_single_cone_production_capability()],
                    required_hir: vec![
                        hir_core_bootstrap_interface_capability(),
                        hir_cross_cone_interface_capability(),
                        hir_identity_foundation_capability(),
                    ],
                    required_mir: vec![
                        mir_core_bootstrap_bridge_capability(),
                        mir_cross_cone_param_free_bridge_capability(),
                        mir_identity_foundation_capability(),
                    ],
                    required_lir: vec![
                        lir_cross_cone_link_closure_capability(),
                        lir_cross_cone_param_free_bridge_capability(),
                        lir_identity_foundation_capability(),
                        lir_link_identity_closure_capability(),
                        lir_strong_production_capability(),
                    ],
                    code_requirement: FingerprintAvailabilityRequirement::MustBeAvailable,
                    runtime_requirement: FingerprintAvailabilityRequirement::MustBeAvailable,
                    publication_class: PublicationClass::Publishable,
                    validation_policy: ArtifactValidationPolicy {
                        odr: OdrValidationPolicy::RejectAll,
                        extra_sections:
                            ExtraSectionPolicy::AllowPurposeDisjointOpaqueAndEnvelopeOptional,
                        link_proof: LinkProofPolicy::Required,
                    },
                }
            }
        }
    }

    pub fn fingerprint(self) -> Result<ArtifactCapabilityProfileFingerprint, HashError> {
        ArtifactCapabilityProfileFingerprint::from_descriptor(&self.descriptor())
    }

    pub(crate) fn validate_link_manifest_inventory(
        self,
        sections: &[crate::ManifestSection],
    ) -> Result<(), ArtifactProfileInventoryError> {
        self.validate_manifest_inventory(ArtifactProfileView::Link, sections)
    }

    pub(crate) fn validate_compile_manifest_inventory(
        self,
        sections: &[crate::ManifestSection],
    ) -> Result<(), ArtifactProfileInventoryError> {
        self.validate_manifest_inventory(ArtifactProfileView::Compile, sections)
    }

    pub(crate) fn validate_link_metadata_inventory(
        self,
        location: crate::MetadataLocation,
        sections: &[crate::DecodedMetadataSection<'_>],
    ) -> Result<(), ArtifactProfileInventoryError> {
        self.validate_metadata_inventory(ArtifactProfileView::Link, location, sections)
    }

    pub(crate) fn validate_compile_metadata_inventory(
        self,
        location: crate::MetadataLocation,
        sections: &[crate::DecodedMetadataSection<'_>],
    ) -> Result<(), ArtifactProfileInventoryError> {
        self.validate_metadata_inventory(ArtifactProfileView::Compile, location, sections)
    }

    fn validate_manifest_inventory(
        self,
        view: ArtifactProfileView,
        sections: &[crate::ManifestSection],
    ) -> Result<(), ArtifactProfileInventoryError> {
        validate_inventory(
            view,
            SectionLocation::Manifest,
            self.descriptor().required_manifest(),
            sections,
            crate::ManifestSection::capability,
            crate::ManifestSection::required_for,
        )
    }

    fn validate_metadata_inventory(
        self,
        view: ArtifactProfileView,
        location: crate::MetadataLocation,
        sections: &[crate::DecodedMetadataSection<'_>],
    ) -> Result<(), ArtifactProfileInventoryError> {
        let descriptor = self.descriptor();
        let expected = match location {
            crate::MetadataLocation::Hir => descriptor.required_hir(),
            crate::MetadataLocation::Mir => descriptor.required_mir(),
            crate::MetadataLocation::Lir => descriptor.required_lir(),
        };
        let section_location = match location {
            crate::MetadataLocation::Hir => SectionLocation::Hir,
            crate::MetadataLocation::Mir => SectionLocation::Mir,
            crate::MetadataLocation::Lir => SectionLocation::Lir,
        };
        validate_inventory(
            view,
            section_location,
            expected,
            sections,
            crate::DecodedMetadataSection::capability,
            crate::DecodedMetadataSection::required_for,
        )
    }
}
