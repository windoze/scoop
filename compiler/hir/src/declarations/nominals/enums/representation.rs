use crate::IntegerKind;

/// The checked source representation, shared by all applications of an enum.
/// Integral values follow the declaration's variant order.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EnumRepresentation {
    Algebraic,
    Integral { kind: IntegerKind, values: Vec<u64> },
}

impl EnumRepresentation {
    pub fn matches_unit_variants(&self, count: usize, all_unit: bool) -> bool {
        match self {
            Self::Algebraic => true,
            Self::Integral { kind, values } => {
                let mut seen = std::collections::BTreeSet::new();
                all_unit
                    && values.len() == count
                    && values
                        .iter()
                        .all(|&value| value <= kind.width().raw_mask() && seen.insert(value))
            }
        }
    }

    pub const fn integer_kind(&self) -> Option<IntegerKind> {
        match self {
            Self::Algebraic => None,
            Self::Integral { kind, .. } => Some(*kind),
        }
    }
}

impl scoop_wire::WireEncode for EnumRepresentation {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
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
                kind.encode(encoder)?;
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

impl scoop_wire::WireDecode for EnumRepresentation {
    fn decode(decoder: &mut scoop_wire::Decoder<'_>) -> Result<Self, scoop_wire::WireError> {
        use scoop_wire::{Decoder, WireError, WireErrorKind};
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let expected = match tag {
            1 => 1,
            2 => 3,
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
        if tag == 1 {
            return Ok(Self::Algebraic);
        }
        let kind = decoder.field(1, IntegerKind::decode)?;
        let values = decoder.field(2, |decoder| {
            decoder.decode_array(|decoder, _| decoder.unsigned())
        })?;
        Ok(Self::Integral { kind, values })
    }
}
