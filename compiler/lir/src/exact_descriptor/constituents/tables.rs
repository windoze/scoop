use super::*;
use crate::{
    ExactDispatchExportV1, StrongTypeDispatchCallableRefV2, StrongTypeItableSemanticPlanV2,
    StrongTypeVtableSemanticPlanV2,
};
use scoop_identity::DispatchTableKey;

pub(super) fn replay(
    source: ExactDescriptorSourceInputV1<'_>,
    dispatch: &CanonicalExactDispatchExportsV1,
) -> Result<
    (
        StrongTypeVtableSemanticPlanV2,
        Vec<StrongTypeItableSemanticPlanV2>,
    ),
    ExactDescriptorError,
> {
    let path = WirePath::root();

    if source
        .interfaces
        .windows(2)
        .any(|pair| pair[0].exact_type() >= pair[1].exact_type())
    {
        return Err(ExactDescriptorError::NonCanonicalInterfaces(source.exact));
    }

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
    let table = find(DispatchTableKey::vtable(source.exact), dispatch)?;
    let vtable =
        StrongTypeVtableSemanticPlanV2::from_artifact(table.table(), slots(table, dispatch)?);
    let mut itables = Vec::new();
    scoop_wire::allocation::try_reserve(&mut itables, source.interfaces.len(), &path)?;
    for &interface in source.interfaces {
        let table = find(
            DispatchTableKey::itable(source.exact, interface.exact_type()),
            dispatch,
        )?;
        itables.push(StrongTypeItableSemanticPlanV2::from_artifact(
            table.table(),
            interface,
            slots(table, dispatch)?,
        ));
    }
    Ok((vtable, itables))
}

fn find(
    key: DispatchTableKey,
    dispatch: &CanonicalExactDispatchExportsV1,
) -> Result<&ExactDispatchExportV1, ExactDescriptorError> {
    let id = scoop_identity::PersistentDispatchTableId::from_key(&key)?;

    dispatch
        .get(id)
        .ok_or(ExactDescriptorError::DispatchTable(id))
}

fn slots(
    table: &ExactDispatchExportV1,
    dispatch: &CanonicalExactDispatchExportsV1,
) -> Result<Vec<StrongTypeDispatchCallableRefV2>, ExactDescriptorError> {
    let path = WirePath::root();

    let mut slots = Vec::new();
    scoop_wire::allocation::try_reserve(&mut slots, table.layout().slots().len(), &path)?;
    for source in table.layout().slots() {
        let origin = dispatch
            .get(source.table)
            .ok_or(ExactDescriptorError::DispatchTable(source.table))?;
        let entry = origin
            .entries()
            .get(source.method as usize)
            .ok_or(ExactDescriptorError::DispatchTable(source.table))?;
        slots.push(entry.abi());
    }
    Ok(slots)
}
