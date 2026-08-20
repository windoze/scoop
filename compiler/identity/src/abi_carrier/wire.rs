use super::*;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

impl WireEncode for AbiCarrier {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, payload) = match self {
            Self::Integer(bits) => (1, u64::from(*bits)),
            Self::Float(FloatKind::F32) => (2, 0),
            Self::Float(FloatKind::F64) => (3, 0),
            Self::Pointer(PointerKind::Managed) => (4, 0),
            Self::Pointer(PointerKind::Raw) => (5, 0),
            Self::Pointer(PointerKind::Code) => (6, 0),
            Self::Pointer(PointerKind::Metadata) => (7, 0),
            Self::FloatPair => (8, 0),
            Self::Array { element, count } => (
                match element {
                    AbiArrayElement::I64 => 9,
                    AbiArrayElement::F32 => 10,
                    AbiArrayElement::F64 => 11,
                },
                u64::from(*count),
            ),
        };
        e.array(2)?;
        e.unsigned(tag)?;
        e.unsigned(payload)
    }
}

fn invalid(d: &Decoder<'_>) -> WireError {
    WireError::new(
        WireErrorKind::IntegerOutOfRange,
        d.path().clone(),
        Some(d.position()),
    )
}

impl WireDecode for AbiCarrier {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        expect_array(d, 2)?;
        let tag = d.unsigned()?;
        let payload = u8::try_from(d.unsigned()?).map_err(|_| invalid(d))?;
        let value = match (tag, payload) {
            (1, bits) => Self::Integer(bits),
            (2, 0) => Self::Float(FloatKind::F32),
            (3, 0) => Self::Float(FloatKind::F64),
            (4, 0) => Self::Pointer(PointerKind::Managed),
            (5, 0) => Self::Pointer(PointerKind::Raw),
            (6, 0) => Self::Pointer(PointerKind::Code),
            (7, 0) => Self::Pointer(PointerKind::Metadata),
            (8, 0) => Self::FloatPair,
            (9..=11, count) => Self::Array {
                element: match tag {
                    9 => AbiArrayElement::I64,
                    10 => AbiArrayElement::F32,
                    _ => AbiArrayElement::F64,
                },
                count,
            },
            _ => return Err(invalid(d)),
        };
        if !value.is_valid() {
            return Err(invalid(d));
        }
        Ok(value)
    }
}

impl WireEncode for AbiPart {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(4)?;
        self.carrier.encode(e)?;
        e.unsigned(self.byte_offset)?;
        e.unsigned(self.extent())?;
        e.unsigned(self.alignment())
    }
}
impl WireDecode for AbiPart {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        expect_array(d, 4)?;
        Self::new(
            AbiCarrier::decode(d)?,
            d.unsigned()?,
            d.unsigned()?,
            d.unsigned()?,
        )
        .map_err(|_| invalid(d))
    }
}
impl WireEncode for AbiCoercion {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(self.parts().len() as u64)?;
        for part in self.parts() {
            part.encode(e)?;
        }
        Ok(())
    }
}
impl WireDecode for AbiCoercion {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        match d.array()? {
            1 => Ok(Self::One(AbiPart::decode(d)?)),
            2 => Ok(Self::Two([AbiPart::decode(d)?, AbiPart::decode(d)?])),
            _ => Err(invalid(d)),
        }
    }
}

fn expect_array(d: &mut Decoder<'_>, expected: u64) -> Result<(), WireError> {
    let actual = d.array()?;
    if actual == expected {
        Ok(())
    } else {
        Err(WireError::new(
            WireErrorKind::InvalidLength { expected, actual },
            d.path().clone(),
            Some(d.position()),
        ))
    }
}
