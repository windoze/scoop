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
pub struct ArtifactCapabilityProfile;

impl ArtifactCapabilityProfile {
    pub const CROSS_CONE_GENERIC: Self = Self;

    pub fn id(self) -> ArtifactCapabilityProfileId {
        ArtifactCapabilityProfileId::cross_cone_generic()
    }

    pub fn from_id(id: &ArtifactCapabilityProfileId) -> Option<Self> {
        (id == &ArtifactCapabilityProfileId::cross_cone_generic()).then_some(Self)
    }

    pub fn descriptor(self) -> ArtifactCapabilityProfileDescriptor {
        let mut descriptor = ArtifactCapabilityProfileDescriptor {
            id: self.id(),
            required_manifest: Vec::new(),
            required_hir: vec![
                hir_core_bootstrap_interface_capability(),
                hir_cross_cone_interface_capability(),
                hir_cross_cone_type_semantics_capability(),
                hir_identity_foundation_capability(),
            ],
            required_mir: vec![
                mir_core_bootstrap_bridge_capability(),
                mir_cross_cone_param_free_bridge_capability(),
                mir_cross_cone_type_bridge_capability(),
                mir_identity_foundation_capability(),
            ],
            required_lir: vec![
                lir_cross_cone_param_free_bridge_capability(),
                lir_identity_foundation_capability(),
                lir_link_identity_closure_capability(),
                lir_cross_cone_layout_abi_capability(),
                lir_cone_production_capability(),
                lir_link_support_capability(),
            ],
        };
        descriptor.required_hir.sort_unstable();
        descriptor.required_mir.sort_unstable();
        descriptor.required_lir.sort_unstable();
        descriptor
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
