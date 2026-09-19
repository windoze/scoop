use super::*;
use scoop_identity::SignatureTypeKey;
use scoop_wire::{WireError, WireErrorKind};

pub(super) fn fields<'a, 'b, I: PartialEq>(
    left: impl ExactSizeIterator<Item = (I, &'a SignatureTypeKey)>,
    right: impl ExactSizeIterator<Item = (I, &'b SignatureTypeKey)>,
    depth: u64,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, WireError> {
    source::sequence(left.len(), meter, path)?;
    source::sequence(right.len(), meter, path)?;
    if left.len() != right.len() {
        return Ok(false);
    }
    for (index, ((left_id, left), (right_id, right))) in left.zip(right).enumerate() {
        let at = path.clone().index(index as u64);
        meter.check_semantic_depth(depth - 1, &at)?;
        meter.charge_work(64, &at)?;
        if left_id != right_id
            || !NominalRepresentationSupportV1::signature_types_match_metered(
                left, right, depth, meter, &at,
            )?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn equal(
    left: &SignatureTypeKey,
    right: &SignatureTypeKey,
    depth: u64,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, WireError> {
    let left_nodes = charge(left, depth, meter, path)?;
    let right_nodes = charge(right, depth, meter, path)?;
    // Prepay comparison scheduling before its borrowed sequence tasks are
    // queued. The shared comparer separately charges its visited work items.
    let scheduling = left_nodes
        .checked_add(right_nodes)
        .and_then(|n| n.checked_mul(3))
        .ok_or_else(|| overflow(path))?;
    meter.charge_work(scheduling, path)?;
    crate::compare_default_signature_reference_targets(left, right, meter, path)
        .map(|ordering| ordering == std::cmp::Ordering::Equal)
}

fn charge(
    signature: &SignatureTypeKey,
    depth: u64,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<u64, WireError> {
    let mut pending = Vec::new();
    push(
        &mut pending,
        std::slice::from_ref(signature),
        depth,
        meter,
        path,
    )?;
    let mut nodes = 0_u64;
    while let Some((signature, depth)) = pending.pop() {
        nodes = nodes.checked_add(1).ok_or_else(|| overflow(path))?;
        meter.charge_nodes(1, path)?;
        meter.charge_work(1, path)?;
        let next = depth.checked_add(1).ok_or_else(|| overflow(path))?;
        match signature {
            SignatureTypeKey::Nominal(_) => meter.charge_work(32, path)?,
            SignatureTypeKey::NominalApplication { arguments, .. } => {
                meter.charge_work(32, path)?;
                push(&mut pending, arguments.as_slice(), next, meter, path)?;
            }
            SignatureTypeKey::Tuple(elements) => {
                push(&mut pending, elements.as_slice(), next, meter, path)?
            }
            SignatureTypeKey::Function {
                parameters, result, ..
            }
            | SignatureTypeKey::NativeFunctionPointer {
                parameters, result, ..
            } => {
                push(&mut pending, parameters, next, meter, path)?;
                push(
                    &mut pending,
                    std::slice::from_ref(result.as_ref()),
                    next,
                    meter,
                    path,
                )?;
            }
            SignatureTypeKey::RawPointer(pointee) => push(
                &mut pending,
                std::slice::from_ref(pointee.as_ref()),
                next,
                meter,
                path,
            )?,
            SignatureTypeKey::Binder { .. } => meter.charge_work(8, path)?,
        }
    }
    Ok(nodes)
}
fn push<'a>(
    pending: &mut Vec<(&'a SignatureTypeKey, u64)>,
    values: &'a [SignatureTypeKey],
    depth: u64,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), WireError> {
    meter.check_table_entries(values.len() as u64, path)?;
    if !values.is_empty() {
        meter.check_semantic_depth(depth, path)?;
    }
    meter.charge_work(values.len() as u64, path)?;
    meter.charge_edges(values.len() as u64, path)?;
    meter.try_reserve_collection_slots(pending, values.len(), path)?;
    pending.extend(values.iter().rev().map(|value| (value, depth)));
    Ok(())
}
fn overflow(path: &WirePath) -> WireError {
    WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None)
}
