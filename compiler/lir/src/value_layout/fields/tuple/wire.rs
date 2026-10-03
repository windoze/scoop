use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::*;

#[derive(Debug)]
pub struct DecodedTupleElementStorageV1 {
    index: u64,
    storage: DecodedFieldStorageV1,
    access_alignment: u64,
}

impl DecodedTupleElementStorageV1 {
    pub(crate) fn link_storage(&self) -> &DecodedFieldStorageV1 {
        &self.storage
    }

    pub fn validate_against(
        self,
        expected: &TupleElementStorageV1,
    ) -> Result<TupleElementStorageV1, TupleStorageReplayError> {
        if self.index != expected.index.ordinal()
            || self.access_alignment != expected.access_alignment.get()
        {
            return Err(TupleStorageReplayError::ElementWireMismatch);
        }
        self.storage.validate_against(&expected.storage)?;
        Ok(expected.clone())
    }
}

impl WireEncode for TupleElementIndexV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(self.ordinal)
    }
}

impl WireEncode for TupleElementStorageV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_element(
            encoder,
            self.index.ordinal,
            &self.storage,
            self.access_alignment.get(),
        )
    }
}

impl WireEncode for DecodedTupleElementStorageV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_element(encoder, self.index, &self.storage, self.access_alignment)
    }
}

impl WireDecode for DecodedTupleElementStorageV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            index: decoder.field(1, Decoder::unsigned)?,
            storage: decoder.field(2, DecodedFieldStorageV1::decode)?,
            access_alignment: decoder.field(3, Decoder::unsigned)?,
        })
    }
}

fn encode_element(
    encoder: &mut Encoder,
    index: u64,
    storage: &impl WireEncode,
    access_alignment: u64,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encoder.field(1)?;
    encoder.unsigned(index)?;
    encoder.field(2)?;
    storage.encode(encoder)?;
    encoder.field(3)?;
    encoder.unsigned(access_alignment)
}
