use crate::RefScan;

use super::RefScanValidationError as Error;

const REFERENCE_BYTES: u64 = 8;

pub(super) fn validate(scan: &RefScan, extent: u64, alignment: u64) -> Result<(), Error> {
    if !matches!(scan, RefScan::None)
        && (alignment < REFERENCE_BYTES || !alignment.is_power_of_two())
    {
        return Err(Error::MisalignedStorage { alignment });
    }
    let mut ranges = Vec::new();
    collect(scan, extent, &mut ranges)?;
    ranges.sort_unstable();
    if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(Error::OverlappingRanges);
    }
    Ok(())
}

fn range(offset: u64, size: u64, extent: u64) -> Result<(u64, u64), Error> {
    if offset % REFERENCE_BYTES != 0 {
        return Err(Error::MisalignedReference { offset });
    }
    let end = offset.checked_add(size).ok_or(Error::OffsetOverflow)?;
    if end > extent {
        return Err(Error::OutOfBounds {
            offset,
            size,
            extent,
        });
    }
    Ok((offset, end))
}

fn collect(scan: &RefScan, extent: u64, ranges: &mut Vec<(u64, u64)>) -> Result<(), Error> {
    match scan {
        RefScan::None => {}
        RefScan::References(offsets) => {
            for offset in offsets {
                ranges.push(range(*offset, REFERENCE_BYTES, extent)?);
            }
        }
        RefScan::Sequence(parts) => {
            for part in parts {
                collect(part, extent, ranges)?;
            }
        }
        RefScan::Array {
            length_offset,
            first_element_offset,
            stride,
            element,
        } => {
            let length = range(*length_offset, REFERENCE_BYTES, extent)?;
            let first = range(*first_element_offset, 0, extent)?;
            if first.0 < length.1 {
                return Err(Error::ArrayPrefixOverlap);
            }
            if stride.get() % REFERENCE_BYTES != 0 {
                return Err(Error::MisalignedArrayStride {
                    stride: stride.get(),
                });
            }
            validate(element.as_ref_scan(), stride.get(), REFERENCE_BYTES)?;
            ranges.push(length);
            // The count is dynamic: reserve the entire possible payload, even
            // when the currently described minimum extent contains no element.
            if first.0 < extent {
                ranges.push((first.0, extent));
            }
        }
    }
    Ok(())
}
