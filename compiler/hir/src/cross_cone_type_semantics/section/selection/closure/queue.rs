use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn enqueue<E>(
    request: SelectedExternalTypeUseV1,
    pending: &mut Vec<SelectedExternalTypeUseV1>,
    visited: &mut BTreeMap<Vec<u8>, SelectedExternalTypeUseV1>,
    external: &mut BTreeMap<Vec<u8>, SelectedExternalTypeUseV1>,
    local: ConeIdentity,

    path: &WirePath,
) -> Result<(), TypeSelectionValidationError<E>> {
    // The selected record is a fixed finite product of typed 32-byte IDs.

    let key = scoop_wire::encode(&request).map_err(TypeSelectionValidationError::Encoding)?;
    if visited.contains_key(&key) {
        return Ok(());
    }

    if request.provider() != local {
        external.insert(key.clone(), request);
    }
    visited.insert(key, request);
    scoop_wire::allocation::try_reserve(pending, 1, path)?;
    pending.push(request);
    Ok(())
}
