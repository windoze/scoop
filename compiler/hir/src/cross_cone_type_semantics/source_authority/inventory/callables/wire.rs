use super::*;
use crate::{
    DecodedDeclarationAccessSourceV1, DecodedInheritanceCallableDeclarationV1,
    DecodedInheritanceCallableSignatureV1,
};
use scoop_wire::{Decoder, WireDecode};

mod resolve;
pub use resolve::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedInheritanceSourceCallableV1 {
    declaration: DecodedInheritanceCallableDeclarationV1,
    signature: DecodedInheritanceCallableSignatureV1,
    modality: CallableModalityV1,
    declaration_access: DecodedDeclarationAccessSourceV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalInheritanceSourceCallablesV1 {
    records: Vec<DecodedInheritanceSourceCallableV1>,
}

macro_rules! encode_record {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(4)?;
                encoder.field(1)?;
                self.declaration.encode(encoder)?;
                encoder.field(2)?;
                self.signature.encode(encoder)?;
                encoder.field(3)?;
                self.modality.encode(encoder)?;
                encoder.field(4)?;
                self.declaration_access.encode(encoder)
            }
        }
    };
}
encode_record!(InheritanceSourceCallableV1);
encode_record!(DecodedInheritanceSourceCallableV1);

impl WireDecode for DecodedInheritanceSourceCallableV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            declaration: decoder.field(1, DecodedInheritanceCallableDeclarationV1::decode)?,
            signature: decoder.field(2, DecodedInheritanceCallableSignatureV1::decode)?,
            modality: decoder.field(3, CallableModalityV1::decode)?,
            declaration_access: decoder.field(4, DecodedDeclarationAccessSourceV1::decode)?,
        })
    }
}
impl WireEncode for DecodedCanonicalInheritanceSourceCallablesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        super::super::wire::sequence(encoder, &self.records)
    }
}
impl WireDecode for DecodedCanonicalInheritanceSourceCallablesV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|d, _| DecodedInheritanceSourceCallableV1::decode(d))
            .map(|records| Self { records })
    }
}
