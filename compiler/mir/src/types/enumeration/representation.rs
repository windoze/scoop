use crate::IntegerKind;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnumRepresentation {
    Algebraic,
    Integral { kind: IntegerKind, values: Vec<u64> },
}

impl EnumRepresentation {
    pub const fn integer_kind(&self) -> Option<IntegerKind> {
        match self {
            Self::Algebraic => None,
            Self::Integral { kind, .. } => Some(*kind),
        }
    }
}

impl WireEncode for EnumRepresentation {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Algebraic => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::Integral { kind, values } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                let index = IntegerKind::ALL
                    .iter()
                    .position(|value| value == kind)
                    .expect("the integer kind belongs to the closed primitive set");
                encoder.unsigned(index as u64 + 1)?;
                encoder.field(2)?;
                encoder.array(values.len() as u64)?;
                for value in values {
                    encoder.unsigned(*value)?;
                }
                Ok(())
            }
        }
    }
}

impl WireDecode for EnumRepresentation {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let expected = match tag {
            1 => 1,
            2 => 3,
            _ => return Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        };
        if fields != expected {
            return Err(error(
                decoder,
                WireErrorKind::InvalidLength {
                    expected,
                    actual: fields,
                },
            ));
        }
        if tag == 1 {
            return Ok(Self::Algebraic);
        }
        let kind = decoder.field(1, |decoder| {
            let tag = decoder.unsigned()?;
            tag.checked_sub(1)
                .and_then(|value| usize::try_from(value).ok())
                .and_then(|index| IntegerKind::ALL.get(index).copied())
                .ok_or_else(|| error(decoder, WireErrorKind::UnknownTag { tag }))
        })?;
        let values = decoder.field(2, |decoder| {
            decoder.decode_array(|decoder, _| decoder.unsigned())
        })?;
        Ok(Self::Integral { kind, values })
    }
}

fn error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
