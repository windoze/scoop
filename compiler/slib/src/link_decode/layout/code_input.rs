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
) -> Result<scoop_lir::DecodedStrongProductionSectionV2, SingleConeLinkSectionDecodeError> {
    let count = graph.envelope.manifest().members().len() as u64;
    graph
        .envelope
        .meter_mut()
        .charge_work(count, &scoop_wire::WirePath::root())
        .map_err(SingleConeLinkSectionDecodeError::Resource)?;
    let location = MetadataLocation::Lir;
    let member = metadata_member_id(graph, location)?;
    let payload = member_payload(graph, location, member)?;
    let envelope = decode_metadata_envelope(graph, payload, location)?;
    graph
        .envelope
        .meter_mut()
        .charge_work(
            envelope.sections().len() as u64,
            &scoop_wire::WirePath::root(),
        )
        .map_err(SingleConeLinkSectionDecodeError::Resource)?;
    let capability = lir_strong_production_v2_capability();
    let payload = required_metadata_section(&envelope, &capability)?;
    decode_inner(graph, location, capability, payload)
}
