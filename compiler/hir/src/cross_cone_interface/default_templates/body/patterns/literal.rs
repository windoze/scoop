use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{
    DecodedDefaultCallableRefV1, DefaultCallableRefResolutionError, DefaultCallableRefV1,
    DefaultCallableReferenceResolver, DefaultIntegerKindV1,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultLiteralEqualityV1 {
    Integer {
        kind: DefaultIntegerKindV1,
        target: DefaultCallableRefV1,
    },
    Ordinary {
        target: DefaultCallableRefV1,
    },
}

impl WireEncode for DefaultLiteralEqualityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Integer { kind, target } => {
                encoder.map(3)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                kind.encode(encoder)?;
                encoder.field(2)?;
                target.encode(encoder)
            }
            Self::Ordinary { target } => {
                encoder.map(2)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                target.encode(encoder)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDefaultLiteralEqualityV1 {
    Integer {
        kind: DefaultIntegerKindV1,
        target: DecodedDefaultCallableRefV1,
    },
    Ordinary {
        target: DecodedDefaultCallableRefV1,
    },
}

impl DecodedDefaultLiteralEqualityV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultLiteralEqualityV1, DefaultLiteralEqualityResolutionError<E>>
    where
        R: DefaultCallableReferenceResolver<E>,
    {
        match self {
            Self::Integer { kind, target } => Ok(DefaultLiteralEqualityV1::Integer {
                kind,
                target: target
                    .resolve(resolver)
                    .map_err(DefaultLiteralEqualityResolutionError::IntegerTarget)?,
            }),
            Self::Ordinary { target } => Ok(DefaultLiteralEqualityV1::Ordinary {
                target: target
                    .resolve(resolver)
                    .map_err(DefaultLiteralEqualityResolutionError::OrdinaryTarget)?,
            }),
        }
    }
}

impl WireEncode for DecodedDefaultLiteralEqualityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Integer { kind, target } => {
                encoder.map(3)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                kind.encode(encoder)?;
                encoder.field(2)?;
                target.encode(encoder)
            }
            Self::Ordinary { target } => {
                encoder.map(2)?;
                encode_tag(encoder, 2)?;
                encoder.field(1)?;
                target.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedDefaultLiteralEqualityV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Integer {
                    kind: decoder.field(1, DefaultIntegerKindV1::decode)?,
                    target: decoder.field(2, DecodedDefaultCallableRefV1::decode)?,
                })
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedDefaultCallableRefV1::decode)
                    .map(|target| Self::Ordinary { target })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultLiteralEqualityResolutionError<E> {
    IntegerTarget(DefaultCallableRefResolutionError<E>),
    OrdinaryTarget(DefaultCallableRefResolutionError<E>),
}

impl<E: fmt::Display> fmt::Display for DefaultLiteralEqualityResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IntegerTarget(error) => {
                write!(
                    formatter,
                    "invalid default integer equality target: {error}"
                )
            }
            Self::OrdinaryTarget(error) => {
                write!(
                    formatter,
                    "invalid default ordinary equality target: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for DefaultLiteralEqualityResolutionError<E>
{
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
