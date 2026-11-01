//! Metadata storage indices share one table across the member's known sections.

use super::*;
use scoop_wire::cbor::{Byte32Index, EncodeError};

pub(super) fn encode(
    envelope: &MetadataEnvelope,
    encoder: &mut Encoder,
) -> Result<(), EncodeError> {
    let indexed = |section: &&MetadataSection| {
        CapabilityContractRegistry::contract(section.capability()).is_some()
    };
    let index = Byte32Index::from_payloads(
        envelope
            .sections
            .iter()
            .filter(indexed)
            .map(MetadataSection::payload),
    )?;
    encoder.map(4)?;
    encoder.field(1)?;
    encoder.bytes(envelope.location.magic())?;
    encoder.field(2)?;
    encoder.unsigned(u64::from(envelope.location.schema()))?;
    encoder.field(3)?;
    encoder.array(envelope.sections.len() as u64)?;
    for section in &envelope.sections {
        let use_index = indexed(&section);
        encoder.map(4)?;
        encoder.field(1)?;
        section.capability.encode(encoder)?;
        encoder.field(2)?;
        section.required_for.encode(encoder)?;
        encoder.field(3)?;
        if use_index {
            encoder.bytes(&index.encode_payload(&section.payload)?)?;
        } else {
            encoder.bytes(&section.payload)?;
        }
        encoder.field(4)?;
        encoder.unsigned(u64::from(use_index))?;
    }
    encoder.field(4)?;
    encoder.bytes(index.table())
}
