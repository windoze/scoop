mod resolution_nodes;

use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{
    DecodedDefaultCallableRefV1, DefaultCallableRefResolutionError, DefaultCallableRefV1,
    DefaultCallableReferenceResolver, DefaultIntegerDivRemV1, DefaultIntegerKindV1,
    DefaultNoGcIntegerOperationV1,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultIntegerOperationV1 {
    NoGc {
        kind: DefaultIntegerKindV1,
        operation: DefaultNoGcIntegerOperationV1,
        target: DefaultCallableRefV1,
    },
    Managed {
        kind: DefaultIntegerKindV1,
        operation: DefaultIntegerDivRemV1,
        target: DefaultCallableRefV1,
    },
}

impl WireEncode for DefaultIntegerOperationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NoGc {
                kind,
                operation,
                target,
            } => encode_operation(encoder, 1, kind, operation, target),
            Self::Managed {
                kind,
                operation,
                target,
            } => encode_operation(encoder, 2, kind, operation, target),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedDefaultIntegerOperationV1 {
    NoGc {
        kind: DefaultIntegerKindV1,
        operation: DefaultNoGcIntegerOperationV1,
        target: DecodedDefaultCallableRefV1,
    },
    Managed {
        kind: DefaultIntegerKindV1,
        operation: DefaultIntegerDivRemV1,
        target: DecodedDefaultCallableRefV1,
    },
}

impl DecodedDefaultIntegerOperationV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<DefaultIntegerOperationV1, DefaultIntegerOperationResolutionError<E>>
    where
        R: DefaultCallableReferenceResolver<E>,
    {
        match self {
            Self::NoGc {
                kind,
                operation,
                target,
            } => Ok(DefaultIntegerOperationV1::NoGc {
                kind,
                operation,
                target: target
                    .resolve(resolver)
                    .map_err(DefaultIntegerOperationResolutionError::NoGcTarget)?,
            }),
            Self::Managed {
                kind,
                operation,
                target,
            } => Ok(DefaultIntegerOperationV1::Managed {
                kind,
                operation,
                target: target
                    .resolve(resolver)
                    .map_err(DefaultIntegerOperationResolutionError::ManagedTarget)?,
            }),
        }
    }
}

impl WireEncode for DecodedDefaultIntegerOperationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::NoGc {
                kind,
                operation,
                target,
            } => encode_operation(encoder, 1, kind, operation, target),
            Self::Managed {
                kind,
                operation,
                target,
            } => encode_operation(encoder, 2, kind, operation, target),
        }
    }
}

impl WireDecode for DecodedDefaultIntegerOperationV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        expect_sum_length(decoder, fields, 4)?;
        match tag {
            1 => Ok(Self::NoGc {
                kind: decoder.field(1, DefaultIntegerKindV1::decode)?,
                operation: decoder.field(2, DefaultNoGcIntegerOperationV1::decode)?,
                target: decoder.field(3, DecodedDefaultCallableRefV1::decode)?,
            }),
            2 => Ok(Self::Managed {
                kind: decoder.field(1, DefaultIntegerKindV1::decode)?,
                operation: decoder.field(2, DefaultIntegerDivRemV1::decode)?,
                target: decoder.field(3, DecodedDefaultCallableRefV1::decode)?,
            }),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultIntegerOperationResolutionError<E> {
    NoGcTarget(DefaultCallableRefResolutionError<E>),
    ManagedTarget(DefaultCallableRefResolutionError<E>),
}

impl<E: fmt::Display> fmt::Display for DefaultIntegerOperationResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoGcTarget(error) => {
                write!(formatter, "invalid no-GC integer operation target: {error}")
            }
            Self::ManagedTarget(error) => {
                write!(
                    formatter,
                    "invalid managed integer operation target: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for DefaultIntegerOperationResolutionError<E>
{
}

fn encode_operation(
    encoder: &mut Encoder,
    tag: u64,
    kind: &impl WireEncode,
    operation: &impl WireEncode,
    target: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(4)?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    kind.encode(encoder)?;
    encoder.field(2)?;
    operation.encode(encoder)?;
    encoder.field(3)?;
    target.encode(encoder)
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

#[cfg(test)]
mod tests;
