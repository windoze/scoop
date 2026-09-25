use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::*;
use crate::cross_cone_type_semantics::wire;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedProtectedDefaultReferenceSetV1 {
    pub(super) callables: Vec<DecodedProtectedDefaultCallableReferenceV1>,
    pub(super) constructors: Vec<DecodedProtectedDefaultConstructorReferenceV1>,
    pub(super) types: Vec<DecodedProtectedDefaultTypeReferenceV1>,
    pub(super) globals: Vec<DecodedProtectedDefaultGlobalReferenceV1>,
    pub(super) singleton_values: Vec<DecodedProtectedDefaultSingletonReferenceV1>,
    pub(super) fields: Vec<DecodedProtectedDefaultFieldReferenceV1>,
}
impl WireDecode for DecodedProtectedDefaultReferenceSetV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            callables: decoder.field(1, |d| {
                d.decode_array(|d, _| DecodedProtectedDefaultCallableReferenceV1::decode(d))
            })?,
            constructors: decoder.field(2, |d| {
                d.decode_array(|d, _| DecodedProtectedDefaultConstructorReferenceV1::decode(d))
            })?,
            types: decoder.field(3, |d| {
                d.decode_array(|d, _| DecodedProtectedDefaultTypeReferenceV1::decode(d))
            })?,
            globals: decoder.field(4, |d| {
                d.decode_array(|d, _| DecodedProtectedDefaultGlobalReferenceV1::decode(d))
            })?,
            singleton_values: decoder.field(5, |d| {
                d.decode_array(|d, _| DecodedProtectedDefaultSingletonReferenceV1::decode(d))
            })?,
            fields: decoder.field(6, |d| {
                d.decode_array(|d, _| DecodedProtectedDefaultFieldReferenceV1::decode(d))
            })?,
        })
    }
}
impl WireEncode for DecodedProtectedDefaultReferenceSetV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        wire::sequence(encoder, &self.callables)?;
        encoder.field(2)?;
        wire::sequence(encoder, &self.constructors)?;
        encoder.field(3)?;
        wire::sequence(encoder, &self.types)?;
        encoder.field(4)?;
        wire::sequence(encoder, &self.globals)?;
        encoder.field(5)?;
        wire::sequence(encoder, &self.singleton_values)?;
        encoder.field(6)?;
        wire::sequence(encoder, &self.fields)
    }
}
