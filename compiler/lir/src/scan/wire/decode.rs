use super::*;

pub(super) fn decode(decoder: &mut Decoder<'_, '_>) -> Result<DecodedRefScanV1, WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    let raw = match tag {
        1 => {
            require_length(decoder, fields, 1)?;

            RawScan::None
        }
        2 => {
            require_length(decoder, fields, 2)?;

            RawScan::References(decoder.field(1, |decoder| {
                let count = decoder.array()?;

                let mut offsets = reserve(decoder, count)?;
                for index in 0..count {
                    offsets.push(decoder.index(index, Decoder::unsigned)?);
                }
                Ok(offsets)
            })?)
        }
        3 => {
            require_length(decoder, fields, 2)?;

            RawScan::Sequence(decoder.field(1, |decoder| {
                let count = decoder.array()?;

                let mut parts = reserve(decoder, count)?;
                for index in 0..count {
                    parts.push(decoder.index(index, decode)?);
                }
                Ok(parts)
            })?)
        }
        4 => {
            require_length(decoder, fields, 5)?;

            RawScan::Array {
                length_offset: decoder.field(1, Decoder::unsigned)?,
                first_element_offset: decoder.field(2, Decoder::unsigned)?,
                stride: decoder.field(3, Decoder::unsigned)?,
                element: decoder.field(4, |decoder| decode(decoder).map(Box::new))?,
            }
        }
        _ => return Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
    };
    Ok(DecodedRefScanV1(raw))
}

fn reserve<T>(decoder: &Decoder<'_, '_>, count: u64) -> Result<Vec<T>, WireError> {
    let capacity = usize::try_from(count)
        .map_err(|_| wire_error(decoder, WireErrorKind::IntegerOutOfRange))?;
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| {
        wire_error(
            decoder,
            WireErrorKind::ResourceAllocation {
                requested_logical_bytes: count.saturating_mul(std::mem::size_of::<T>() as u64),
                requested_slots: count,
            },
        )
    })?;
    Ok(values)
}
