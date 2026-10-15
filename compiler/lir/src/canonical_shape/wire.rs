use super::*;
use scoop_identity::DecodedPersistentId;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

impl WireEncode for CanonicalShapeAbisV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(self.definitions.len() as u64)?;
        for definition in &self.definitions {
            encode_record(e, &definition.member(), definition.abi)?;
        }
        Ok(())
    }
}

fn encode_record(
    e: &mut Encoder,
    member: &impl WireEncode,
    abi: Digest256,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    e.map(2)?;
    e.field(1)?;
    member.encode(e)?;
    e.field(3)?;
    abi.encode(e)
}

#[derive(Debug)]
pub struct DecodedCanonicalShapeAbisV1 {
    definitions: Vec<(DecodedPersistentId<OdrMemberId>, Digest256)>,
}

impl WireDecode for DecodedCanonicalShapeAbisV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        let definitions = d.decode_array(|d, _| {
            d.expect_map(2)?;
            Ok((
                d.field(1, DecodedPersistentId::decode)?,
                d.field(3, Digest256::decode)?,
            ))
        })?;
        Ok(Self { definitions })
    }
}

impl WireEncode for DecodedCanonicalShapeAbisV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(self.definitions.len() as u64)?;
        for (member, abi) in &self.definitions {
            encode_record(e, member, *abi)?;
        }
        Ok(())
    }
}
