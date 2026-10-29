//! Constant payloads interpreted under the containing property's exact signature.

use scoop_identity::PersistentEnumVariantId;
use scoop_wire::{Encoder, WireEncode};

use super::CanonicalConstValueV1;

mod decoded;
pub use decoded::DecodedValueConstantV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalValueConstantV1 {
    Scalar(CanonicalConstValueV1),
    Unit,
    Tuple(Vec<Self>),
    Struct(Vec<Self>),
    Enum {
        variant: PersistentEnumVariantId,
        fields: Vec<Self>,
    },
}

impl From<CanonicalConstValueV1> for CanonicalValueConstantV1 {
    fn from(value: CanonicalConstValueV1) -> Self {
        Self::Scalar(value)
    }
}

impl WireEncode for CanonicalValueConstantV1 {
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

fn encode_header(
    encoder: &mut Encoder,
    tag: u64,
    fields: u64,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(fields)?;
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_elements(
    encoder: &mut Encoder,
    tag: u64,
    elements: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encode_header(encoder, tag, 2)?;
    encoder.field(1)?;
    encode_sequence(encoder, elements)
}

fn encode_variant(
    encoder: &mut Encoder,
    variant: &impl WireEncode,
    fields: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encode_header(encoder, 9, 3)?;
    encoder.field(1)?;
    variant.encode(encoder)?;
    encoder.field(2)?;
    encode_sequence(encoder, fields)
}

fn encode_sequence(
    encoder: &mut Encoder,
    elements: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(elements.len() as u64)?;
    for element in elements {
        element.encode(encoder)?;
    }
    Ok(())
}
