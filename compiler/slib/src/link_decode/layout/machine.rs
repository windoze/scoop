//! Decode only canonical keys and machine records for program linking.
use super::super::{
    decode_metadata_envelope, member_payload, metadata_member_id, require_strong_profile,
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
    pub layout: DecodedCrossConeLayoutAbiSectionV1,
    pub link: DecodedCrossConeLayoutLinkOnlySections,
}

impl<'a> ValidatedGraphArtifact<'a> {
    pub(crate) fn decode_machine_link_sections(
        self,
    ) -> Result<DecodedMachineLinkSections<'a>, SingleConeLinkSectionDecodeError> {
        let profile = require_strong_profile(&self, ArtifactCapabilityProfile::CROSS_CONE_GENERIC)?;
        profile
            .validate_link_manifest_inventory(self.envelope.manifest().sections())
            .map_err(SingleConeLinkSectionDecodeError::Inventory)?;
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
        let layout = decode_required(
            &lir,
            MetadataLocation::Lir,
            lir_cross_cone_layout_abi_capability(),
        )?;
        let link = DecodedCrossConeLayoutLinkOnlySections {
            link_support: decode_required(
                &lir,
                MetadataLocation::Lir,
                lir_link_support_capability(),
            )?,
            link_identity_closure: decode_required(
                &lir,
                MetadataLocation::Lir,
                lir_link_identity_closure_capability(),
            )?,
        };
        Ok(DecodedMachineLinkSections {
            graph: self,
            hir_keys,
            mir_keys,
            foundation,
            production,
            layout,
            link,
        })
    }
}
