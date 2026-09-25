use scoop_wire::{BudgetMeter, WireError, WirePath};

pub(super) fn table<T>(
    count: usize,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), WireError> {
    let count = count as u64;
    meter.check_table_entries(count, path)?;
    meter.charge_collection_slots(count, path)?;
    meter.charge_owned_bytes(count.saturating_mul(std::mem::size_of::<T>() as u64), path)?;
    meter.charge_work(
        count.saturating_mul(1 + u64::from(count.max(1).ilog2())),
        path,
    )
}

pub(super) fn allocate<T>(
    count: usize,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<Vec<T>, WireError> {
    let mut values = Vec::new();
    meter.check_table_entries(count as u64, path)?;
    meter.charge_work(
        (count as u64).saturating_mul(1 + u64::from(count.max(1).ilog2())),
        path,
    )?;
    meter.try_reserve_exact(
        &mut values,
        count as u64,
        std::mem::size_of::<T>() as u64,
        path,
    )?;
    Ok(values)
}
