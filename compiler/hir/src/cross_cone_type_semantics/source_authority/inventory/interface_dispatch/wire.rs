use super::super::wire as codec;
use super::*;
use crate::DecodedCanonicalPersistentIdsV1;
use scoop_identity::DecodedPersistentId;
use scoop_wire::{Decoder, WireDecode, WireError};

mod resolve;
pub use resolve::InterfaceSourceMemberResolutionError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedInterfaceSourceMemberV1 {
    slot: DecodedPersistentId<PersistentDispatchSlotId>,
    overrides: DecodedCanonicalPersistentIdsV1<PersistentDispatchSlotId>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedInterfaceSourceDispatchV1 {
    owner: DecodedPersistentId<PersistentExactTypeId>,
    parents: Vec<DecodedPersistentId<PersistentExactTypeId>>,
    members: Vec<DecodedInterfaceSourceMemberV1>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalInterfaceSourceDispatchesV1 {
    records: Vec<DecodedInterfaceSourceDispatchV1>,
}

macro_rules! encode_member {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(2)?;
                encoder.field(1)?;
                self.slot.encode(encoder)?;
                encoder.field(2)?;
                self.overrides.encode(encoder)
            }
        }
    };
}
encode_member!(InterfaceSourceMemberV1);
encode_member!(DecodedInterfaceSourceMemberV1);
macro_rules! encode_record {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(3)?;
                encoder.field(1)?;
                self.owner.encode(encoder)?;
                encoder.field(2)?;
                codec::sequence(encoder, &self.parents)?;
                encoder.field(3)?;
                codec::sequence(encoder, &self.members)
            }
        }
    };
}
encode_record!(InterfaceSourceDispatchV1);
encode_record!(DecodedInterfaceSourceDispatchV1);

impl WireDecode for DecodedInterfaceSourceMemberV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            slot: decoder.field(1, DecodedPersistentId::decode)?,
            overrides: decoder.field(2, DecodedCanonicalPersistentIdsV1::decode)?,
        })
    }
}
impl WireDecode for DecodedInterfaceSourceDispatchV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            owner: decoder.field(1, DecodedPersistentId::decode)?,
            parents: decoder.field(2, |d| d.decode_array(|d, _| DecodedPersistentId::decode(d)))?,
            members: decoder.field(3, |d| {
                d.decode_array(|d, _| DecodedInterfaceSourceMemberV1::decode(d))
            })?,
        })
    }
}
impl WireDecode for DecodedCanonicalInterfaceSourceDispatchesV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedInterfaceSourceDispatchV1::decode(d))
            .map(|records| Self { records })
    }
}
impl WireEncode for DecodedCanonicalInterfaceSourceDispatchesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        codec::sequence(encoder, &self.records)
    }
}
