use super::*;
use scoop_identity::{DispatchDeclarationOwner, PropertyOwner};

pub(super) fn collect(
    context: &mut SchemaDeclarations<'_>,
    metadata: SharedTypeMetadataV1<'_>,
    slot: PersistentDispatchSlotId,
) -> Result<(), Error> {
    if context.slots.contains_key(&slot) {
        return Ok(());
    }

    let identities = metadata.identities;

    let key = identities.canonical_key::<_, DispatchSlotKey>(slot)?;
    match key.owner() {
        DispatchDeclarationOwner::Function(id) => {
            context
                .functions
                .insert(id, identities.canonical_key::<_, SourceDeclarationKey>(id)?);
        }
        DispatchDeclarationOwner::Accessor(id) => {
            let accessor = identities.canonical_key::<_, PropertyAccessorKey>(id)?;
            let PropertyOwner::Property(property) = accessor.owner() else {
                return Err(Error::SlotSource(slot));
            };

            context.properties.insert(
                property,
                identities.canonical_key::<_, SourceDeclarationKey>(property)?,
            );
            context.accessors.insert(id, accessor);
        }
    }

    context.slots.insert(slot, key);
    Ok(())
}
