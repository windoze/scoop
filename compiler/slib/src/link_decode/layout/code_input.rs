//! Reborrow the original Strong payload for Code without exporting replay state.

use super::super::{
    decode_inner, decode_metadata_envelope, member_payload, metadata_member_id,
    required_metadata_section,
};
use crate::{
    MetadataLocation, SingleConeLinkSectionDecodeError, ValidatedGraphArtifact,
    lir_strong_production_v2_capability,
};

pub(crate) fn layout_code_strong_input(
    graph: &mut ValidatedGraphArtifact<'_>,
) -> Result<scoop_lir::DecodedConeProductionSectionV2, SingleConeLinkSectionDecodeError> {
    let location = MetadataLocation::Lir;
    let member = metadata_member_id(graph, location)?;
    let payload = member_payload(graph, location, member)?;
    let envelope = decode_metadata_envelope(payload, location)?;

    let capability = lir_strong_production_v2_capability();
    let payload = required_metadata_section(&envelope, &capability)?;
    decode_inner(location, capability, payload)
}
