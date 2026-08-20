use super::*;
use scoop_wire::WireErrorKind;

impl WireEncode for TypeUseStep {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, index) = match *self {
            Self::Argument(i) => (1, Some(i)),
            Self::TupleElement(i) => (2, Some(i)),
            Self::FunctionReceiver => (3, None),
            Self::FunctionParameter(i) => (4, Some(i)),
            Self::FunctionResult => (5, None),
            Self::PointerPointee => (6, None),
            Self::FunctionPointee => (7, None),
        };
        e.array(1 + u64::from(index.is_some()))?;
        e.unsigned(tag)?;
        if let Some(index) = index {
            e.unsigned(index.into())?;
        }
        Ok(())
    }
}
impl WireDecode for TypeUseStep {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        let length = d.array()?;
        let tag = d.index(0, Decoder::unsigned)?;
        let expected = if matches!(tag, 1 | 2 | 4) { 2 } else { 1 };
        check_length(d, length, expected)?;
        Ok(match tag {
            1 => Self::Argument(d.index(1, Decoder::u32)?),
            2 => Self::TupleElement(d.index(1, Decoder::u32)?),
            3 => Self::FunctionReceiver,
            4 => Self::FunctionParameter(d.index(1, Decoder::u32)?),
            5 => Self::FunctionResult,
            6 => Self::PointerPointee,
            7 => Self::FunctionPointee,
            tag => return Err(error(d, WireErrorKind::UnknownTag { tag })),
        })
    }
}

impl WireEncode for AnnotationTypeSlot {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match *self {
            Self::Value | Self::Result => {
                e.array(1)?;
                e.unsigned(if *self == Self::Value { 1 } else { 2 })
            }
            Self::Supertype(i) | Self::AnnotationParameter(i) => {
                e.array(2)?;
                e.unsigned(if matches!(self, Self::Supertype(_)) {
                    3
                } else {
                    5
                })?;
                e.unsigned(i.into())
            }
            Self::Bound { parameter, index } => {
                e.array(3)?;
                e.unsigned(4)?;
                e.unsigned(parameter.into())?;
                e.unsigned(index.into())
            }
        }
    }
}
impl WireDecode for AnnotationTypeSlot {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        let length = d.array()?;
        let tag = d.index(0, Decoder::unsigned)?;
        check_length(
            d,
            length,
            match tag {
                3 | 5 => 2,
                4 => 3,
                _ => 1,
            },
        )?;
        Ok(match tag {
            1 => Self::Value,
            2 => Self::Result,
            3 => Self::Supertype(d.index(1, Decoder::u32)?),
            4 => Self::Bound {
                parameter: d.index(1, Decoder::u32)?,
                index: d.index(2, Decoder::u32)?,
            },
            5 => Self::AnnotationParameter(d.index(1, Decoder::u32)?),
            tag => return Err(error(d, WireErrorKind::UnknownTag { tag })),
        })
    }
}

fn check_length(d: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(error(d, WireErrorKind::InvalidLength { expected, actual }))
    }
}
fn error(d: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, d.path().clone(), Some(d.position()))
}
