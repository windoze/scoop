use super::*;
use scoop_identity::PersistentDispatchSlotId;

pub(super) fn validate<E>(
    local: &Exports<'_>,
    provider: &Exports<'_>,
    receiver: PersistentExactTypeId,
    slot: PersistentDispatchSlotId,

    path: &WirePath,
) -> Result<PersistentExactTypeId, Error<E>> {
    let receiver_record = local
        .inheritance_records
        .get(&receiver)
        .ok_or(Error::Slot)?;
    let actual = receiver_record.slots().get(slot).ok_or(Error::Slot)?;
    let owner = exact(SourceNominalId::Concrete(actual.declaration_owner()))?;
    let root_record = provider.inheritance.table().get(owner).ok_or(Error::Slot)?;
    let root = root_record.slots().get(slot).ok_or(Error::Slot)?;

    let access = root.declaration_access();
    let actual_access = actual.declaration_access();

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
    if !contains(receiver_record.slot_schemas(), slot)?
        || !contains(root_record.slot_schemas(), slot)?
    {
        return Err(Error::Slot);
    }
    receiver::validate(&local.graph, receiver, owner, path)?;
    Ok(owner)
}

fn contains(
    schemas: &CanonicalInheritanceSlotSchemasV1,
    slot: PersistentDispatchSlotId,
) -> Result<bool, scoop_wire::WireError> {
    for schema in schemas.records() {
        if schema.slots().contains(&slot) {
            return Ok(true);
        }
    }
    Ok(false)
}
