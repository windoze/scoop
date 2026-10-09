use super::*;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

impl<T: WireEncode> WireEncode for MaybeUninitOperation<T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(if self.operand().is_some() { 2 } else { 1 })?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Uninit => 1,
            Self::Initialized(_) => 2,
            Self::AssumeInit(_) => 3,
        })?;
        if let Some(value) = self.operand() {
            encoder.field(1)?;
            value.encode(encoder)?;
        }
        Ok(())
    }
}

impl<T: WireDecode> WireDecode for MaybeUninitOperation<T> {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let expected = match tag {
            1 => 1,
            2 | 3 => 2,
            _ => {
                return Err(WireError::new(
                    WireErrorKind::UnknownTag { tag },
                    decoder.path().clone(),
                    Some(decoder.position()),
                ));
            }
        };
        if fields != expected {
            return Err(WireError::new(
                WireErrorKind::InvalidLength {
                    expected,
                    actual: fields,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        Ok(match tag {
            1 => Self::Uninit,
            2 => Self::Initialized(decoder.field(1, T::decode)?),
            3 => Self::AssumeInit(decoder.field(1, T::decode)?),
            _ => unreachable!("the sum tag was checked above"),
        })
    }
}
