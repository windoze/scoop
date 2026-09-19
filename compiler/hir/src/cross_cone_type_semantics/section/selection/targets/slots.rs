use super::*;
use scoop_identity::PersistentDispatchSlotId;

pub(super) fn validate<E>(
    local: &Exports<'_>,
    provider: &Exports<'_>,
    receiver: PersistentExactTypeId,
    slot: PersistentDispatchSlotId,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<PersistentExactTypeId, Error<E>> {
    meter.charge_work(
        (local.inheritance_records.len() as u64 + 1).ilog2() as u64 + 1,
        path,
    )?;
    let receiver_record = local
        .inheritance_records
        .get(&receiver)
        .ok_or(Error::Slot)?;
    let actual = receiver_record.slots().get(slot).ok_or(Error::Slot)?;
    let owner = exact(
        SourceNominalId::Concrete(actual.declaration_owner()),
        meter,
        path,
    )?;
    let root_record = provider.inheritance.table().get(owner).ok_or(Error::Slot)?;
    let root = root_record.slots().get(slot).ok_or(Error::Slot)?;
    meter.charge_work(
        (actual.signature().exact_signature().parameters().len()
            + root.signature().exact_signature().parameters().len()) as u64
            + 8,
        path,
    )?;
    let access = root.declaration_access();
    let actual_access = actual.declaration_access();
    resources::access(access, meter, path)?;
    resources::access(actual_access, meter, path)?;
    resources::domain(root.domain().domain(), meter, path)?;
    resources::domain(actual.domain().domain(), meter, path)?;
    if root.declaration_owner() != actual.declaration_owner()
        || root.declaration() != actual.declaration()
        || root.signature() != actual.signature()
        || root.domain() != actual.domain()
        || access != actual_access
        || access.definition_origin().origin().source().cone() != provider.provider
    {
        return Err(Error::Slot);
    }
    // Each checked interface already proves schema/slot exact coverage. Keep
    // both schema memberships explicit at the terminal selection boundary.
    if !contains(receiver_record.slot_schemas(), slot, meter, path)?
        || !contains(root_record.slot_schemas(), slot, meter, path)?
    {
        return Err(Error::Slot);
    }
    receiver::validate(&local.graph, receiver, owner, meter, path)?;
    Ok(owner)
}

fn contains(
    schemas: &CanonicalInheritanceSlotSchemasV1,
    slot: PersistentDispatchSlotId,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<bool, scoop_wire::WireError> {
    meter.check_table_entries(schemas.records().len() as u64, path)?;
    for schema in schemas.records() {
        meter.charge_work(schema.slots().len() as u64, path)?;
        if schema.slots().contains(&slot) {
            return Ok(true);
        }
    }
    Ok(false)
}
