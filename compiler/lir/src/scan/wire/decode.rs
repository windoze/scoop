use scoop_wire::ResourceKind;

use super::*;
use crate::ScanBudgetResourceV1 as R;

#[derive(Default)]
struct Budget {
    nodes: u64,
    words: u64,
    bytes: u64,
}

pub(super) fn decode(decoder: &mut Decoder<'_, '_>) -> Result<DecodedRefScanV1, WireError> {
    node(decoder, &mut Budget::default(), 1)
}

fn limit(decoder: &Decoder<'_, '_>, resource: R, actual: u64) -> Result<(), WireError> {
    if actual <= resource.maximum() {
        return Ok(());
    }
    let kind = match resource {
        R::Depth => ResourceKind::SemanticRecursion,
        R::DistinctNodes | R::ExpandedNodes => ResourceKind::DecodedNodes,
        R::DistinctWords => ResourceKind::SemanticTableEntries,
        R::CanonicalBytes => ResourceKind::SemanticLeafBytes,
    };
    Err(wire_error(
        decoder,
        WireErrorKind::LimitExceeded {
            resource: kind,
            limit: resource.maximum(),
            observed: actual,
        },
    ))
}

fn charge(
    decoder: &Decoder<'_, '_>,
    used: &mut u64,
    count: u64,
    resource: R,
) -> Result<(), WireError> {
    let actual = used
        .checked_add(count)
        .ok_or_else(|| wire_error(decoder, WireErrorKind::IntegerOutOfRange))?;
    limit(decoder, resource, actual)?;
    *used = actual;
    Ok(())
}

fn node(
    decoder: &mut Decoder<'_, '_>,
    budget: &mut Budget,
    depth: u64,
) -> Result<DecodedRefScanV1, WireError> {
    limit(decoder, R::Depth, depth)?;
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    if tag != 1 {
        charge(decoder, &mut budget.nodes, 1, R::DistinctNodes)?;
    }
    // Wire is a tree, so each decoded node is also one expanded occurrence.
    limit(decoder, R::ExpandedNodes, budget.nodes)?;
    let raw = match tag {
        1 => {
            require_length(decoder, fields, 1)?;
            charge(decoder, &mut budget.bytes, 4, R::CanonicalBytes)?;
            RawScan::None
        }
        2 => {
            require_length(decoder, fields, 2)?;
            charge(decoder, &mut budget.words, 1, R::DistinctWords)?;
            charge(decoder, &mut budget.bytes, 12, R::CanonicalBytes)?;
            RawScan::References(decoder.field(1, |decoder| {
                let count = decoder.array()?;
                charge(decoder, &mut budget.words, count, R::DistinctWords)?;
                let bytes = count
                    .checked_mul(8)
                    .ok_or_else(|| wire_error(decoder, WireErrorKind::IntegerOutOfRange))?;
                charge(decoder, &mut budget.bytes, bytes, R::CanonicalBytes)?;
                let mut offsets = reserve(decoder, count)?;
                for index in 0..count {
                    offsets.push(decoder.index(index, Decoder::unsigned)?);
                }
                Ok(offsets)
            })?)
        }
        3 => {
            require_length(decoder, fields, 2)?;
            charge(decoder, &mut budget.words, 2, R::DistinctWords)?;
            charge(decoder, &mut budget.bytes, 12, R::CanonicalBytes)?;
            RawScan::Sequence(decoder.field(1, |decoder| {
                let count = decoder.array()?;
                charge(decoder, &mut budget.words, count, R::DistinctWords)?;
                // Even a None child consumes canonical bytes. Charge this
                // lower bound before reserving a maliciously large sequence.
                let minimum = count
                    .checked_mul(4)
                    .and_then(|bytes| budget.bytes.checked_add(bytes))
                    .ok_or_else(|| wire_error(decoder, WireErrorKind::IntegerOutOfRange))?;
                limit(decoder, R::CanonicalBytes, minimum)?;
                let mut parts = reserve(decoder, count)?;
                for index in 0..count {
                    parts.push(decoder.index(index, |decoder| node(decoder, budget, depth + 1))?);
                }
                Ok(parts)
            })?)
        }
        4 => {
            require_length(decoder, fields, 5)?;
            charge(decoder, &mut budget.words, 5, R::DistinctWords)?;
            charge(decoder, &mut budget.bytes, 28, R::CanonicalBytes)?;
            RawScan::Array {
                length_offset: decoder.field(1, Decoder::unsigned)?,
                first_element_offset: decoder.field(2, Decoder::unsigned)?,
                stride: decoder.field(3, Decoder::unsigned)?,
                element: decoder
                    .field(4, |decoder| node(decoder, budget, depth + 1).map(Box::new))?,
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
