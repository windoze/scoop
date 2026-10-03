use crate::{NonEmptyRefScan, RefScan};

use super::{RefScanValidationError as Error, fingerprint};

pub(super) fn canonical_bytes(scan: &RefScan) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    match scan {
        RefScan::None => bytes.extend(0_u32.to_le_bytes()),
        RefScan::References(offsets) => {
            if offsets.is_empty() {
                return Err(Error::EmptyReferences);
            }
            if !offsets.windows(2).all(|pair| pair[0] < pair[1]) {
                return Err(Error::UnorderedReferences);
            }
            bytes.extend(1_u32.to_le_bytes());
            bytes.extend((offsets.len() as u64).to_le_bytes());
            for offset in offsets {
                bytes.extend(offset.to_le_bytes());
            }
        }
        RefScan::Sequence(parts) => {
            if parts.len() < 2 {
                return Err(Error::NonCanonicalSequence);
            }
            bytes.extend(2_u32.to_le_bytes());
            bytes.extend((parts.len() as u64).to_le_bytes());
            let mut previous = None;
            let mut references = false;
            for part in parts {
                match part {
                    RefScan::None | RefScan::Sequence(_) => {
                        return Err(Error::NonCanonicalSequence);
                    }
                    RefScan::References(_) if references => {
                        return Err(Error::NonCanonicalSequence);
                    }
                    RefScan::References(_) => references = true,
                    RefScan::Array { .. } => {}
                }
                let child = canonical_bytes(part)?;
                let key = (fingerprint(&child), child.as_slice());
                if previous
                    .as_ref()
                    .is_some_and(|(hash, bytes): &(_, Vec<u8>)| (*hash, bytes.as_slice()) >= key)
                {
                    return Err(Error::NonCanonicalSequence);
                }
                bytes.extend_from_slice(&child);
                previous = Some((key.0, child));
            }
        }
        RefScan::Array {
            length_offset,
            first_element_offset,
            stride,
            element,
        } => {
            let child = element.as_ref_scan();
            if !child.contains_reference() {
                return Err(Error::EmptyArrayElement);
            }
            bytes.extend(3_u32.to_le_bytes());
            bytes.extend(length_offset.to_le_bytes());
            bytes.extend(first_element_offset.to_le_bytes());
            bytes.extend(stride.get().to_le_bytes());
            bytes.extend(canonical_bytes(child)?);
        }
    }
    Ok(bytes)
}

pub(super) fn normalize(scan: RefScan) -> Result<RefScan, Error> {
    match scan {
        RefScan::None => Ok(RefScan::None),
        RefScan::References(mut offsets) => {
            offsets.sort_unstable();
            offsets.dedup();
            Ok(if offsets.is_empty() {
                RefScan::None
            } else {
                RefScan::References(offsets)
            })
        }
        RefScan::Sequence(parts) => normalize_sequence(parts),
        RefScan::Array {
            length_offset,
            first_element_offset,
            stride,
            element,
        } => {
            let element = normalize(element.as_ref_scan().clone())?;
            let Some(element) = NonEmptyRefScan::new(element) else {
                return Ok(RefScan::None);
            };
            Ok(RefScan::Array {
                length_offset,
                first_element_offset,
                stride,
                element: Box::new(element),
            })
        }
    }
}

fn normalize_sequence(parts: Vec<RefScan>) -> Result<RefScan, Error> {
    fn collect(scan: RefScan, refs: &mut Vec<u64>, children: &mut Vec<RefScan>) {
        match scan {
            RefScan::None => {}
            RefScan::References(offsets) => refs.extend(offsets),
            RefScan::Sequence(parts) => {
                for part in parts {
                    collect(part, refs, children);
                }
            }
            array @ RefScan::Array { .. } => children.push(array),
        }
    }
    let mut refs = Vec::new();
    let mut children = Vec::new();
    for part in parts {
        collect(normalize(part)?, &mut refs, &mut children);
    }
    if !refs.is_empty() {
        children.push(normalize(RefScan::References(refs))?);
    }
    let mut children = children
        .into_iter()
        .map(|scan| {
            let bytes = canonical_bytes(&scan)?;
            Ok((fingerprint(&bytes), bytes, scan))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    children.sort_unstable_by(|left, right| (&left.0, &left.1).cmp(&(&right.0, &right.1)));
    children.dedup_by(|left, right| left.0 == right.0 && left.1 == right.1);
    let mut children: Vec<_> = children.into_iter().map(|(_, _, scan)| scan).collect();
    Ok(match children.len() {
        0 => RefScan::None,
        1 => children.remove(0),
        _ => RefScan::Sequence(children),
    })
}

pub(super) fn translate(scan: &RefScan, delta: u64) -> Result<RefScan, Error> {
    let shift = |offset: u64| offset.checked_add(delta).ok_or(Error::OffsetOverflow);
    match scan {
        RefScan::None => Ok(RefScan::None),
        RefScan::References(offsets) => offsets
            .iter()
            .map(|offset| shift(*offset))
            .collect::<Result<Vec<_>, _>>()
            .map(RefScan::References),
        RefScan::Sequence(parts) => parts
            .iter()
            .map(|part| translate(part, delta))
            .collect::<Result<Vec<_>, _>>()
            .map(RefScan::Sequence),
        RefScan::Array {
            length_offset,
            first_element_offset,
            stride,
            element,
        } => Ok(RefScan::Array {
            length_offset: shift(*length_offset)?,
            first_element_offset: shift(*first_element_offset)?,
            stride: *stride,
            element: element.clone(),
        }),
    }
}
