use super::*;
use crate::{
    ExactDispatchExportV1, StrongTypeDispatchCallableRefV2, StrongTypeItableSemanticPlanV2,
    StrongTypeVtableSemanticPlanV2,
};
use scoop_identity::DispatchTableKey;

pub(super) fn replay(
    source: ExactDescriptorSourceInputV1<'_>,
    dispatch: &CanonicalExactDispatchExportsV1,
    meter: &mut BudgetMeter,
) -> Result<
    (
        StrongTypeVtableSemanticPlanV2,
        Vec<StrongTypeItableSemanticPlanV2>,
    ),
    ExactDescriptorError,
> {
    let path = WirePath::root();
    meter.check_table_entries(source.interfaces.len() as u64, &path)?;
    meter.charge_work(source.interfaces.len() as u64, &path)?;
    if source
        .interfaces
        .windows(2)
        .any(|pair| pair[0].exact_type() >= pair[1].exact_type())
    {
        return Err(ExactDescriptorError::NonCanonicalInterfaces(source.exact));
    }
    meter.charge_work(dispatch.records().len() as u64, &path)?;
    let count = dispatch
        .records()
        .iter()
        .filter(|table| table.owner_exact() == source.exact)
        .count();
    if count
        != source
            .interfaces
            .len()
            .checked_add(1)
            .ok_or(ExactDescriptorError::CountOverflow)?
    {
        return Err(ExactDescriptorError::DispatchInventory(source.exact));
    }
    let table = find(DispatchTableKey::vtable(source.exact), dispatch, meter)?;
    let vtable = StrongTypeVtableSemanticPlanV2::from_artifact(table.table(), slots(table, meter)?);
    let mut itables = Vec::new();
    meter.try_reserve_collection_slots(&mut itables, source.interfaces.len(), &path)?;
    for &interface in source.interfaces {
        let table = find(
            DispatchTableKey::itable(source.exact, interface.exact_type()),
            dispatch,
            meter,
        )?;
        itables.push(StrongTypeItableSemanticPlanV2::from_artifact(
            table.table(),
            interface,
            slots(table, meter)?,
        ));
    }
    Ok((vtable, itables))
}

fn find<'a>(
    key: DispatchTableKey,
    dispatch: &'a CanonicalExactDispatchExportsV1,
    meter: &mut BudgetMeter,
) -> Result<&'a ExactDispatchExportV1, ExactDescriptorError> {
    let id = scoop_identity::PersistentDispatchTableId::from_key(&key)?;
    meter.charge_work(
        u64::from(dispatch.records().len().max(1).ilog2()) + 1,
        &WirePath::root(),
    )?;
    dispatch
        .get(id)
        .ok_or(ExactDescriptorError::DispatchTable(id))
}

fn slots(
    table: &ExactDispatchExportV1,
    meter: &mut BudgetMeter,
) -> Result<Vec<StrongTypeDispatchCallableRefV2>, ExactDescriptorError> {
    let path = WirePath::root();
    meter.charge_work(table.entries().len() as u64, &path)?;
    let mut slots = Vec::new();
    meter.try_reserve_collection_slots(&mut slots, table.entries().len(), &path)?;
    slots.extend(table.entries().iter().map(|entry| entry.abi()));
    Ok(slots)
}
