use super::*;

pub(super) fn validate(
    records: &[ExternalHirReferenceV1],
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), ExternalHirReferenceSetBuildError> {
    use ExternalHirReferenceSetBuildError as Error;
    let mut positions = Vec::new();
    meter
        .charge_work(records.len() as u64, path)
        .map_err(Error::Resource)?;
    for record in records {
        for site in record.call_sites().records() {
            meter
                .charge_owned_bytes(
                    std::mem::size_of::<crate::concrete::ExecutableExpressionPosition>() as u64,
                    path,
                )
                .map_err(Error::Resource)?;
            meter
                .try_reserve_collection_slots(&mut positions, 1, path)
                .map_err(Error::Resource)?;
            positions.push(site.position());
        }
    }
    let count = positions.len() as u64;
    meter
        .charge_work(
            count.saturating_mul(2 + u64::from(count.max(1).ilog2())),
            path,
        )
        .map_err(Error::Resource)?;
    positions.sort_unstable();
    if let Some(pair) = positions.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(Error::DuplicateCallPosition(pair[1]));
    }
    Ok(())
}
