use crate::{
    ArtifactCapabilityProfile, MetadataLocation, SingleConeLinkSectionDecodeError,
    ValidatedGraphArtifact, hir_core_bootstrap_interface_capability,
    hir_cross_cone_interface_capability, hir_cross_cone_type_semantics_capability,
    hir_identity_foundation_capability, lir_cone_production_capability,
    lir_cross_cone_layout_abi_capability, lir_cross_cone_layout_link_closure_capability,
    lir_cross_cone_link_closure_capability, lir_cross_cone_param_free_bridge_capability,
    lir_identity_foundation_capability, lir_link_identity_closure_capability,
    mir_core_bootstrap_bridge_capability, mir_cross_cone_param_free_bridge_capability,
    mir_cross_cone_type_bridge_capability, mir_identity_foundation_capability,
};

use super::super::{
    decode_inner, decode_metadata_envelope, decode_production_manifest, member_payload,
    metadata_member_id, require_strong_profile, required_metadata_section,
    validate_semantic_fingerprints,
};
use super::DecodedCrossConeLayoutLinkSections;

impl<'input> ValidatedGraphArtifact<'input> {
    /// Decodes the complete metadata inventory needed by later M23-6 Link
    /// validation without promoting any decoded field to semantic authority.
    pub fn decode_cross_cone_layout_link_sections(
        mut self,
    ) -> Result<DecodedCrossConeLayoutLinkSections<'input>, CrossConeLayoutLinkSectionDecodeError>
    {
        let profile = require_strong_profile(&self, ArtifactCapabilityProfile::CROSS_CONE_GENERIC)?;
        profile
            .validate_compile_manifest_inventory(self.envelope.manifest().sections())
            .map_err(SingleConeLinkSectionDecodeError::Inventory)?;
        let production_manifest = decode_production_manifest(&mut self, profile)?;

        let hir_member = metadata_member_id(&self, MetadataLocation::Hir)?;
        let mir_member = metadata_member_id(&self, MetadataLocation::Mir)?;
        let lir_member = metadata_member_id(&self, MetadataLocation::Lir)?;
        let hir_payload = member_payload(&self, MetadataLocation::Hir, hir_member)?;
        let mir_payload = member_payload(&self, MetadataLocation::Mir, mir_member)?;
        let lir_payload = member_payload(&self, MetadataLocation::Lir, lir_member)?;
        let hir = decode_metadata_envelope(hir_payload, MetadataLocation::Hir)?;
        let mir = decode_metadata_envelope(mir_payload, MetadataLocation::Mir)?;
        let lir = decode_metadata_envelope(lir_payload, MetadataLocation::Lir)?;

        validate_inventories(profile, &hir, &mir, &lir)?;

        let hir_foundation = decode_required(
            &hir,
            MetadataLocation::Hir,
            hir_identity_foundation_capability(),
        )?;
        let hir_core_production = decode_required(
            &hir,
            MetadataLocation::Hir,
            hir_core_bootstrap_interface_capability(),
        )?;
        let hir_interface = decode_required(
            &hir,
            MetadataLocation::Hir,
            hir_cross_cone_interface_capability(),
        )?;
        let hir_type_semantics = decode_required(
            &hir,
            MetadataLocation::Hir,
            hir_cross_cone_type_semantics_capability(),
        )?;
        let mir_foundation = decode_required(
            &mir,
            MetadataLocation::Mir,
            mir_identity_foundation_capability(),
        )?;
        let mir_core_production = decode_required(
            &mir,
            MetadataLocation::Mir,
            mir_core_bootstrap_bridge_capability(),
        )?;
        let mir_cross_cone_bridge = decode_required(
            &mir,
            MetadataLocation::Mir,
            mir_cross_cone_param_free_bridge_capability(),
        )?;
        let mir_type_bridge = decode_required(
            &mir,
            MetadataLocation::Mir,
            mir_cross_cone_type_bridge_capability(),
        )?;
        let lir_foundation = decode_required(
            &lir,
            MetadataLocation::Lir,
            lir_identity_foundation_capability(),
        )?;
        let lir_strong_production = decode_required(
            &lir,
            MetadataLocation::Lir,
            lir_cone_production_capability(),
        )?;
        let lir_cross_cone_bridge = decode_required(
            &lir,
            MetadataLocation::Lir,
            lir_cross_cone_param_free_bridge_capability(),
        )?;
        let lir_layout_abi = decode_required(
            &lir,
            MetadataLocation::Lir,
            lir_cross_cone_layout_abi_capability(),
        )?;
        let link_identity_closure = decode_required(
            &lir,
            MetadataLocation::Lir,
            lir_link_identity_closure_capability(),
        )?;
        let cross_cone_link_closure = decode_required(
            &lir,
            MetadataLocation::Lir,
            lir_cross_cone_link_closure_capability(),
        )?;
        let layout_link_closure = decode_required(
            &lir,
            MetadataLocation::Lir,
            lir_cross_cone_layout_link_closure_capability(),
        )?;

        // Only the three Compile semantic contributions are checked here.
        // Code and LinkValidationOnly projections require final-object proofs.
        validate_semantic_fingerprints(&mut self, &hir, &mir, &lir)?;

        Ok(DecodedCrossConeLayoutLinkSections {
            graph: self,
            production_manifest,
            hir_foundation,
            hir_core_production,
            hir_interface,
            hir_type_semantics,
            mir_foundation,
            mir_core_production,
            mir_cross_cone_bridge,
            mir_type_bridge,
            lir_foundation,
            lir_strong_production,
            lir_cross_cone_bridge,
            lir_layout_abi,
            link_identity_closure,
            cross_cone_link_closure,
            layout_link_closure,
        })
    }
}

fn validate_inventories(
    profile: ArtifactCapabilityProfile,
    hir: &crate::DecodedMetadataEnvelope<'_>,
    mir: &crate::DecodedMetadataEnvelope<'_>,
    lir: &crate::DecodedMetadataEnvelope<'_>,
) -> Result<(), SingleConeLinkSectionDecodeError> {
    for (location, envelope) in [
        (MetadataLocation::Hir, hir),
        (MetadataLocation::Mir, mir),
        (MetadataLocation::Lir, lir),
    ] {
        profile
            .validate_compile_metadata_inventory(location, envelope.sections())
            .map_err(SingleConeLinkSectionDecodeError::Inventory)?;
        profile
            .validate_link_metadata_inventory(location, envelope.sections())
            .map_err(SingleConeLinkSectionDecodeError::Inventory)?;
    }
    Ok(())
}

fn decode_required<T: scoop_wire::WireDecode>(
    envelope: &crate::DecodedMetadataEnvelope<'_>,
    location: MetadataLocation,
    capability: scoop_identity::CapabilityId,
) -> Result<T, SingleConeLinkSectionDecodeError> {
    let payload = required_metadata_section(envelope, &capability)?;
    decode_inner(location, capability, payload)
}

pub type CrossConeLayoutLinkSectionDecodeError = SingleConeLinkSectionDecodeError;
