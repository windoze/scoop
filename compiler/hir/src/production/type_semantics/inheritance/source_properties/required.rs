use super::*;
use scoop_identity::{CallableTemplateOrigin, PersistentPropertyAccessorId};
use std::collections::BTreeSet;

pub(super) fn project(
    export: &ExportHir,
    inventory: &CanonicalSourceInheritanceInventoriesV1,
    selections: &CanonicalInheritanceSourceSlotSelectionsV1,
) -> Result<BTreeSet<PersistentPropertyId>, Error> {
    let mut properties = BTreeSet::new();
    let mut accessors: BTreeSet<PersistentPropertyAccessorId> = BTreeSet::new();
    for owner in inventory.records() {
        for member in owner.protected_members().values() {
            match member {
                ProtectedDeclarationRefV1::Property(id) => insert(&mut properties, *id)?,
                ProtectedDeclarationRefV1::Callable(id) => {
                    if let CallableTemplateOrigin::Accessor(id) = id.declaration() {
                        insert(&mut accessors, id)?;
                    }
                }
                ProtectedDeclarationRefV1::Constructor(_)
                | ProtectedDeclarationRefV1::NestedNominal(_) => continue,
            }
        }
    }
    for declaration in super::super::source_callables::required(export, inventory, selections)? {
        match declaration {
            InheritanceCallableDeclarationV1::Function(_) => continue,
            InheritanceCallableDeclarationV1::Getter(id)
            | InheritanceCallableDeclarationV1::Setter(id) => insert(&mut accessors, id)?,
        }
    }
    for (id, property) in export.properties.iter() {
        let getter = export.property_accessor_identities[property.capability.getter()].id();
        let getter_required = accessors.remove(&getter);
        let setter_required = if let Some(setter) = property.capability.setter() {
            accessors.remove(&export.property_accessor_identities[setter].id())
        } else {
            false
        };
        if getter_required || setter_required {
            let HirPropertyIdentity::Ordinary(identity) = &export.property_identities[id] else {
                return Err(invalid(
                    "inheritance accessor belongs to an extension property",
                ));
            };
            insert(&mut properties, identity.id())?;
        }
    }
    if !accessors.is_empty() {
        return Err(invalid(
            "required inheritance accessor has no sealed logical property",
        ));
    }
    Ok(properties)
}

fn insert<T: Ord>(set: &mut BTreeSet<T>, value: T) -> Result<(), Error> {
    if !set.contains(&value) {
        set.insert(value);
    }
    Ok(())
}
