//! Decode only canonical keys and machine records for program linking.
use super::super::{
    decode_metadata_envelope, decode_production_manifest, member_payload, metadata_member_id,
    require_strong_profile, validate_semantic_fingerprints,
};
use super::decode::{decode_required, validate_inventories};
use super::*;
use crate::*;

pub(crate) struct DecodedMachineLinkSections<'a> {
    pub graph: ValidatedGraphArtifact<'a>,
    pub hir_keys: DecodedHirFoundation,
    pub mir_keys: DecodedMirFoundation,
    pub foundation: DecodedLirFoundation,
    pub production: DecodedConeProductionSectionV2,
    pub ordinary: DecodedCrossConeLirBridgeSectionV1,
    pub layout: DecodedCrossConeLayoutAbiSectionV1,
    pub link: DecodedCrossConeLayoutLinkOnlySections,
}

impl<'a> ValidatedGraphArtifact<'a> {
    pub(crate) fn decode_machine_link_sections(
        mut self,
    ) -> Result<DecodedMachineLinkSections<'a>, SingleConeLinkSectionDecodeError> {
        let profile = require_strong_profile(&self, ArtifactCapabilityProfile::CROSS_CONE_GENERIC)?;
        let production_manifest = decode_production_manifest(&mut self, profile)?;
        let hir_id = metadata_member_id(&self, MetadataLocation::Hir)?;
        let mir_id = metadata_member_id(&self, MetadataLocation::Mir)?;
        let lir_id = metadata_member_id(&self, MetadataLocation::Lir)?;
        let hir = decode_metadata_envelope(
            member_payload(&self, MetadataLocation::Hir, hir_id)?,
            MetadataLocation::Hir,
        )?;
        let mir = decode_metadata_envelope(
            member_payload(&self, MetadataLocation::Mir, mir_id)?,
            MetadataLocation::Mir,
        )?;
        let lir = decode_metadata_envelope(
            member_payload(&self, MetadataLocation::Lir, lir_id)?,
            MetadataLocation::Lir,
        )?;
        validate_inventories(profile, &hir, &mir, &lir)?;
        let hir_keys = decode_required(
            &hir,
            MetadataLocation::Hir,
            hir_identity_foundation_capability(),
        )?;
        let mir_keys = decode_required(
            &mir,
            MetadataLocation::Mir,
            mir_identity_foundation_capability(),
        )?;
        let foundation = decode_required(
            &lir,
            MetadataLocation::Lir,
            lir_identity_foundation_capability(),
        )?;
        let production = decode_required(
            &lir,
            MetadataLocation::Lir,
            lir_cone_production_capability(),
        )?;
        let ordinary = decode_required(
            &lir,
            MetadataLocation::Lir,
            lir_cross_cone_param_free_bridge_capability(),
        )?;
        let layout = decode_required(
            &lir,
            MetadataLocation::Lir,
            lir_cross_cone_layout_abi_capability(),
        )?;
        let link = DecodedCrossConeLayoutLinkOnlySections {
            production_manifest,
            link_identity_closure: decode_required(
                &lir,
                MetadataLocation::Lir,
                lir_link_identity_closure_capability(),
            )?,
            cross_cone_link_closure: decode_required(
                &lir,
                MetadataLocation::Lir,
                lir_cross_cone_link_closure_capability(),
            )?,
            layout_link_closure: decode_required(
                &lir,
                MetadataLocation::Lir,
                lir_cross_cone_layout_link_closure_capability(),
            )?,
        };
        // Hash the immutable payloads; this does not decode source semantics.
        validate_semantic_fingerprints(&mut self, &hir, &mir, &lir)?;
        Ok(DecodedMachineLinkSections {
            graph: self,
            hir_keys,
            mir_keys,
            foundation,
            production,
            ordinary,
            layout,
            link,
        })
    }
}
