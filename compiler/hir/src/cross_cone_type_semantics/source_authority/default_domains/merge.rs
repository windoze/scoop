use super::*;

pub(super) fn intersect(
    left: &DefaultSourceAccessDomainV1,
    right: &DefaultSourceAccessDomainV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<DefaultSourceAccessDomainV1, Error> {
    meter.charge_work(1, path)?;
    if left.is_empty() || right.is_empty() {
        return Ok(DefaultSourceAccessDomainV1::empty());
    }
    let count = left.persistent().constraints().len() as u64
        + right.persistent().constraints().len() as u64;
    meter.check_table_entries(count, path)?;
    for _ in 0..4 {
        meter.charge_collection_slots(count, path)?;
    }
    for constraint in left
        .persistent()
        .constraints()
        .iter()
        .chain(right.persistent().constraints())
    {
        let bytes = scoop_wire::encoded_length(constraint).map_err(Error::Encoding)?;
        meter.check_semantic_leaf(bytes, path)?;
        meter.charge_owned_bytes(bytes.saturating_mul(4), path)?;
        meter.charge_work(
            bytes.saturating_mul(count + u64::from(count.max(1).ilog2()) + 4),
            path,
        )?;
    }
    let persistent = left
        .persistent()
        .intersect(right.persistent())
        .map_err(Error::PersistentDomain)?;
    let count =
        left.generic_subclasses().values().len() + right.generic_subclasses().values().len();
    meter.check_table_entries(count as u64, path)?;
    meter.charge_work(
        (count as u64)
            .saturating_mul(2 * u64::from(count.max(1).ilog2()) + 4)
            .saturating_mul(65),
        path,
    )?;
    let mut generic = Vec::new();
    meter.try_reserve_collection_slots(&mut generic, count, path)?;
    generic.extend_from_slice(left.generic_subclasses().values());
    generic.extend_from_slice(right.generic_subclasses().values());
    generic.sort_unstable();
    generic.dedup();
    let generic = CanonicalPersistentIdsV1::try_new(generic).map_err(Error::GenericDomain)?;
    DefaultSourceAccessDomainV1::try_new(persistent, generic).map_err(Error::DomainBuild)
}
