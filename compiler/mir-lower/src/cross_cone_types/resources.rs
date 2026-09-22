use super::*;

pub(super) fn work(
    count: usize,
    meter: &mut BudgetMeter,
) -> Result<(), SourceMirTypeProductionError> {
    meter
        .charge_work(count as u64, &WirePath::root())
        .map_err(SourceMirTypeProductionError::Resource)
}

pub(super) fn sort(
    count: usize,
    meter: &mut BudgetMeter,
) -> Result<(), SourceMirTypeProductionError> {
    work(
        count.saturating_mul(count.checked_ilog2().unwrap_or(0) as usize + 1),
        meter,
    )
}

pub(super) fn reserve<T>(
    values: &mut Vec<T>,
    count: usize,
    meter: &mut BudgetMeter,
) -> Result<(), SourceMirTypeProductionError> {
    let path = WirePath::root();
    meter
        .charge_owned_bytes(
            (count as u64).saturating_mul(std::mem::size_of::<T>() as u64),
            &path,
        )
        .map_err(SourceMirTypeProductionError::Resource)?;
    meter
        .try_reserve_collection_slots(values, count, &path)
        .map_err(SourceMirTypeProductionError::Resource)
}
