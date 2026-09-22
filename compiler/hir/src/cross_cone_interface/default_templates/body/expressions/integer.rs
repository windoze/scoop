mod resolution_nodes;

use crate::{DefaultIntegerDivRemV1, DefaultIntegerKindV1, DefaultNoGcIntegerOperationV1};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefaultIntegerOperationV1 {
    NoGc {
        kind: DefaultIntegerKindV1,
        operation: DefaultNoGcIntegerOperationV1,
    },
    Managed {
        kind: DefaultIntegerKindV1,
        operation: DefaultIntegerDivRemV1,
    },
}

pub type DecodedDefaultIntegerOperationV1 = DefaultIntegerOperationV1;

impl From<crate::IntegerOperation> for DefaultIntegerOperationV1 {
    fn from(value: crate::IntegerOperation) -> Self {
        match value {
            crate::IntegerOperation::NoGc { kind, operation } => Self::NoGc {
                kind: kind.into(),
                operation: operation.into(),
            },
            crate::IntegerOperation::Managed { kind, operation } => Self::Managed {
                kind: kind.into(),
                operation: operation.into(),
            },
        }
    }
}

impl From<DefaultIntegerOperationV1> for crate::IntegerOperation {
    fn from(value: DefaultIntegerOperationV1) -> Self {
        match value {
            DefaultIntegerOperationV1::NoGc { kind, operation } => Self::NoGc {
                kind: kind.into(),
                operation: operation.into(),
            },
            DefaultIntegerOperationV1::Managed { kind, operation } => Self::Managed {
                kind: kind.into(),
                operation: operation.into(),
            },
        }
    }
}

impl WireEncode for DefaultIntegerOperationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::NoGc { .. } => 1,
            Self::Managed { .. } => 2,
        })?;
        match self {
            Self::NoGc { kind, operation } => {
                encoder.field(1)?;
                kind.encode(encoder)?;
                encoder.field(2)?;
                operation.encode(encoder)
            }
            Self::Managed { kind, operation } => {
                encoder.field(1)?;
                kind.encode(encoder)?;
                encoder.field(2)?;
                operation.encode(encoder)
            }
        }
    }
}

impl WireDecode for DefaultIntegerOperationV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        expect_sum_length(decoder, fields, 3)?;
        match tag {
            1 => Ok(Self::NoGc {
                kind: decoder.field(1, DefaultIntegerKindV1::decode)?,
                operation: decoder.field(2, DefaultNoGcIntegerOperationV1::decode)?,
            }),
            2 => Ok(Self::Managed {
                kind: decoder.field(1, DefaultIntegerKindV1::decode)?,
                operation: decoder.field(2, DefaultIntegerDivRemV1::decode)?,
            }),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
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
