use scoop_identity::{DecodedPersistentId, PersistentEnumVariantId, PersistentIdResolver};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::super::{expect_sum_length, wire_error};
use super::{
    CanonicalConstValueV1, CanonicalValueConstantV1, encode_elements, encode_header, encode_variant,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedValueConstantV1 {
    Scalar(CanonicalConstValueV1),
    Unit,
    Tuple(Vec<Self>),
    Struct(Vec<Self>),
    Enum {
        variant: DecodedPersistentId<PersistentEnumVariantId>,
        fields: Vec<Self>,
    },
}

impl DecodedValueConstantV1 {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<CanonicalValueConstantV1, E>
    where
        R: PersistentIdResolver<PersistentEnumVariantId, Error = E>,
    {
        use CanonicalValueConstantV1 as C;
        Ok(match self {
            Self::Scalar(value) => C::Scalar(value),
            Self::Unit => C::Unit,
            Self::Tuple(elements) => C::Tuple(resolve_elements(elements, resolver)?),
            Self::Struct(fields) => C::Struct(resolve_elements(fields, resolver)?),
            Self::Enum { variant, fields } => C::Enum {
                variant: resolver.resolve(variant)?,
                fields: resolve_elements(fields, resolver)?,
            },
        })
    }
}

fn resolve_elements<R, E>(
    elements: Vec<DecodedValueConstantV1>,
    resolver: &mut R,
) -> Result<Vec<CanonicalValueConstantV1>, E>
where
    R: PersistentIdResolver<PersistentEnumVariantId, Error = E>,
{
    elements
        .into_iter()
        .map(|element| element.resolve(resolver))
        .collect()
}

impl WireEncode for DecodedValueConstantV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Scalar(value) => value.encode(encoder),
            Self::Unit => encode_header(encoder, 6, 1),
            Self::Tuple(elements) => encode_elements(encoder, 7, elements),
            Self::Struct(fields) => encode_elements(encoder, 8, fields),
            Self::Enum { variant, fields } => encode_variant(encoder, variant, fields),
        }
    }
}

impl WireDecode for DecodedValueConstantV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        expect_sum_length(
            decoder,
            fields,
            match tag {
                6 => 1,
                9 => 3,
                _ => 2,
            },
        )?;
        match tag {
            1..=5 => CanonicalConstValueV1::decode_payload(tag, decoder).map(Self::Scalar),
            6 => Ok(Self::Unit),
            7 => decoder.field(1, decode_elements).map(Self::Tuple),
            8 => decoder.field(1, decode_elements).map(Self::Struct),
            9 => Ok(Self::Enum {
                variant: decoder.field(1, DecodedPersistentId::decode)?,
                fields: decoder.field(2, decode_elements)?,
            }),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

fn decode_elements(decoder: &mut Decoder<'_>) -> Result<Vec<DecodedValueConstantV1>, WireError> {
    decoder.decode_array(|decoder, _| DecodedValueConstantV1::decode(decoder))
}
