use std::fmt;

use scoop_identity::{DecodedPersistentId, PersistentIdResolver, PersistentPropertyId};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultStringOwnerV1 {
    CurrentInstantiation,
    Property(PersistentPropertyId),
    PropertyElement {
        property: PersistentPropertyId,
        ordinal: u32,
    },
}

impl WireEncode for DefaultStringOwnerV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::CurrentInstantiation => encode_empty_sum(encoder, 1),
            Self::Property(property) => encode_single_payload(encoder, 2, property),
            Self::PropertyElement { property, ordinal } => {
                encode_property_element(encoder, property, *ordinal)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedDefaultStringOwnerV1 {
    CurrentInstantiation,
    Property(DecodedPersistentId<PersistentPropertyId>),
    PropertyElement {
        property: DecodedPersistentId<PersistentPropertyId>,
        ordinal: u32,
    },
}

impl DecodedDefaultStringOwnerV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultStringOwnerV1, DefaultStringOwnerResolutionError<E>>
    where
        R: PersistentIdResolver<PersistentPropertyId, Error = E>,
    {
        match self {
            Self::CurrentInstantiation => Ok(DefaultStringOwnerV1::CurrentInstantiation),
            Self::Property(property) => resolver
                .resolve(property)
                .map(DefaultStringOwnerV1::Property)
                .map_err(DefaultStringOwnerResolutionError::Property),
            Self::PropertyElement { property, ordinal } => resolver
                .resolve(property)
                .map(|property| DefaultStringOwnerV1::PropertyElement { property, ordinal })
                .map_err(DefaultStringOwnerResolutionError::Property),
        }
    }
}

impl WireEncode for DecodedDefaultStringOwnerV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::CurrentInstantiation => encode_empty_sum(encoder, 1),
            Self::Property(property) => encode_single_payload(encoder, 2, property),
            Self::PropertyElement { property, ordinal } => {
                encode_property_element(encoder, property, *ordinal)
            }
        }
    }
}

impl WireDecode for DecodedDefaultStringOwnerV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::CurrentInstantiation)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedPersistentId::decode)
                    .map(Self::Property)
            }
            3 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::PropertyElement {
                    property: decoder.field(1, DecodedPersistentId::decode)?,
                    ordinal: decoder.field(2, |decoder| {
                        u32::try_from(decoder.unsigned()?)
                            .map_err(|_| wire_error(decoder, WireErrorKind::IntegerOutOfRange))
                    })?,
                })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultStringOwnerResolutionError<E> {
    Property(E),
}

impl<E: fmt::Display> fmt::Display for DefaultStringOwnerResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Property(error) => write!(formatter, "invalid default string owner: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for DefaultStringOwnerResolutionError<E> {}

fn encode_property_element(
    encoder: &mut Encoder,
    property: &impl WireEncode,
    ordinal: u32,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, 3)?;
    encoder.field(1)?;
    property.encode(encoder)?;
    encoder.field(2)?;
    encoder.unsigned(u64::from(ordinal))
}

fn encode_single_payload(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;
