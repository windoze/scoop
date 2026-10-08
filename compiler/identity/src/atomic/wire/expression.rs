use super::*;

impl<T: WireEncode> WireEncode for AtomicExpression<T> {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(3)?;
        e.field(0)?;
        self.kind.encode(e)?;
        e.field(1)?;
        self.object.encode(e)?;
        e.field(2)?;
        self.operation.encode(e)
    }
}

impl<T: WireDecode> WireDecode for AtomicExpression<T> {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(3)?;
        Ok(Self {
            kind: d.field(0, AtomicValueKind::decode)?,
            object: d.field(1, T::decode)?,
            operation: d.field(2, AtomicOperation::decode)?,
        })
    }
}

impl<T: WireEncode> WireEncode for AtomicOperation<T> {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, fields) = match self {
            Self::Load { .. } => (0, 2),
            Self::Store { .. } => (1, 3),
            Self::Rmw { .. } => (2, 4),
            Self::CompareExchange { .. } => (3, 5),
        };
        e.map(fields)?;
        e.field(0)?;
        e.unsigned(tag)?;
        e.field(1)?;
        match self {
            Self::Load { order } => order.encode(e),
            Self::Store { value, order } => {
                order.encode(e)?;
                e.field(2)?;
                value.encode(e)
            }
            Self::Rmw {
                value,
                operation,
                order,
            } => {
                order.encode(e)?;
                e.field(2)?;
                operation.encode(e)?;
                e.field(3)?;
                value.encode(e)
            }
            Self::CompareExchange {
                expected,
                value,
                order,
                result,
            } => {
                order.encode(e)?;
                e.field(2)?;
                result.encode(e)?;
                e.field(3)?;
                expected.encode(e)?;
                e.field(4)?;
                value.encode(e)
            }
        }
    }
}

impl<T: WireDecode> WireDecode for AtomicOperation<T> {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = d.map()?;
        if fields == 0 {
            return Err(error(d, WireErrorKind::MissingField { field: 0 }));
        }
        let tag = d.field(0, Decoder::unsigned)?;
        let count = match tag {
            0..=3 => tag + 2,
            tag => return Err(error(d, WireErrorKind::UnknownTag { tag })),
        };
        if fields != count {
            return Err(error(
                d,
                WireErrorKind::InvalidLength {
                    expected: count,
                    actual: fields,
                },
            ));
        }
        Ok(match tag {
            0 => Self::Load {
                order: d.field(1, AtomicLoadOrder::decode)?,
            },
            1 => Self::Store {
                order: d.field(1, AtomicStoreOrder::decode)?,
                value: d.field(2, T::decode)?,
            },
            2 => Self::Rmw {
                order: d.field(1, AtomicMemoryOrder::decode)?,
                operation: d.field(2, AtomicRmwOperation::decode)?,
                value: d.field(3, T::decode)?,
            },
            3 => Self::CompareExchange {
                order: d.field(1, AtomicCompareExchangeOrder::decode)?,
                result: d.field(2, AtomicCompareExchangeResult::decode)?,
                expected: d.field(3, T::decode)?,
                value: d.field(4, T::decode)?,
            },
            _ => unreachable!("the wire tag was checked above"),
        })
    }
}

fn error(d: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, d.path().clone(), Some(d.position()))
}
