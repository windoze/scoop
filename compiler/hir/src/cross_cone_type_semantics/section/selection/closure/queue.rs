use super::*;
pub(super) struct Queued {
    pub request: SelectedExternalTypeUseV1,
    pub depth: u64,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn enqueue<E>(
    queued: Queued,
    pending: &mut Vec<Queued>,
    visited: &mut BTreeMap<Vec<u8>, SelectedExternalTypeUseV1>,
    external: &mut BTreeMap<Vec<u8>, SelectedExternalTypeUseV1>,
    local: ConeIdentity,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeSelectionValidationError<E>> {
    let request = queued.request;
    // The selected record is a fixed finite product of typed 32-byte IDs.
    let bytes =
        scoop_wire::encoded_length(&request).map_err(TypeSelectionValidationError::Encoding)?;
    meter.charge_owned_bytes(bytes, path)?;
    meter.charge_work(
        bytes.saturating_mul((visited.len() as u64 + 1).ilog2() as u64 + 2),
        path,
    )?;
    let key = scoop_wire::encode(&request).map_err(TypeSelectionValidationError::Encoding)?;
    if visited.contains_key(&key) {
        return Ok(());
    }
    meter.check_semantic_depth(queued.depth, path)?;
    meter.check_table_entries(visited.len() as u64 + 1, path)?;
    meter.charge_collection_slots(1, path)?;
    if request.provider() != local {
        meter.charge_collection_slots(1, path)?;
        meter.charge_owned_bytes(bytes, path)?;
        external.insert(key.clone(), request);
    }
    visited.insert(key, request);
    meter.try_reserve_collection_slots(pending, 1, path)?;
    pending.push(queued);
    Ok(())
}
