use scoop_wire::{WireError, WirePath};

pub(super) fn allocate<T>(count: usize, path: &WirePath) -> Result<Vec<T>, WireError> {
    let mut values = Vec::new();

    scoop_wire::allocation::try_reserve_count(&mut values, count as u64, path)?;
    Ok(values)
}
