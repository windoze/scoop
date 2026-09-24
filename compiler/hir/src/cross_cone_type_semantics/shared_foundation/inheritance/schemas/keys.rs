use super::*;
use scoop_identity::{DispatchDeclarationOwner, PropertyOwner};

pub(super) fn collect(
    context: &mut SchemaDeclarations<'_>,
    metadata: SharedTypeMetadataV1<'_>,
    slot: PersistentDispatchSlotId,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    contracts::lookup(context.slots.len(), meter)?;
    if context.slots.contains_key(&slot) {
        return Ok(());
    }
    let path = WirePath::root();
    let identities = metadata.identities;
    contracts::lookup(identities.identity_count(), meter)?;
    let key = identities.canonical_key::<_, DispatchSlotKey>(slot)?;
    match key.owner() {
        DispatchDeclarationOwner::Function(id) => {
            contracts::lookup(identities.identity_count(), meter)?;
            meter.charge_collection_slots(1, &path)?;
            context
                .functions
                .insert(id, identities.canonical_key::<_, SourceDeclarationKey>(id)?);
        }
        DispatchDeclarationOwner::Accessor(id) => {
            contracts::lookup(identities.identity_count(), meter)?;
            let accessor = identities.canonical_key::<_, PropertyAccessorKey>(id)?;
            let PropertyOwner::Property(property) = accessor.owner() else {
                return Err(Error::SlotSource(slot));
            };
            contracts::lookup(identities.identity_count(), meter)?;
            meter.charge_collection_slots(2, &path)?;
            context.properties.insert(
                property,
                identities.canonical_key::<_, SourceDeclarationKey>(property)?,
            );
            context.accessors.insert(id, accessor);
        }
    }
    meter.charge_collection_slots(1, &path)?;
    context.slots.insert(slot, key);
    Ok(())
}
