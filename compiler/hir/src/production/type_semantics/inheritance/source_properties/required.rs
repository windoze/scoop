use super::*;
use scoop_identity::{CallableTemplateOrigin, PersistentPropertyAccessorId};
use std::collections::BTreeSet;

pub(super) fn project(
    export: &ExportHir,
    inventory: &CanonicalSourceInheritanceInventoriesV1,
    selections: &CanonicalInheritanceSourceSlotSelectionsV1,
    meter: &mut BudgetMeter,
) -> Result<BTreeSet<PersistentPropertyId>, Error> {
    let mut properties = BTreeSet::new();
    let mut accessors: BTreeSet<PersistentPropertyAccessorId> = BTreeSet::new();
    for owner in inventory.records() {
        work(meter, 1)?;
        for member in owner.protected_members().values() {
            work(meter, 1)?;
            match member {
                ProtectedDeclarationRefV1::Property(id) => insert(&mut properties, *id, meter)?,
                ProtectedDeclarationRefV1::Callable(id) => {
                    if let CallableTemplateOrigin::Accessor(id) = id.declaration() {
                        insert(&mut accessors, id, meter)?;
                    }
                }
                ProtectedDeclarationRefV1::Constructor(_)
                | ProtectedDeclarationRefV1::NestedNominal(_) => continue,
            }
        }
    }
    for declaration in
        super::super::source_callables::required(export, inventory, selections, meter)?
    {
        match declaration {
            InheritanceCallableDeclarationV1::Function(_) => continue,
            InheritanceCallableDeclarationV1::Getter(id)
            | InheritanceCallableDeclarationV1::Setter(id) => insert(&mut accessors, id, meter)?,
        }
    }
    for (id, property) in export.properties.iter() {
        work(meter, accessors.len())?;
        let getter = export.property_accessor_identities[property.capability.getter()].id();
        let getter_required = accessors.remove(&getter);
        let setter_required = if let Some(setter) = property.capability.setter() {
            work(meter, accessors.len())?;
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
            insert(&mut properties, identity.id(), meter)?;
        }
    }
    if !accessors.is_empty() {
        return Err(invalid(
            "required inheritance accessor has no sealed logical property",
        ));
    }
    Ok(properties)
}

fn insert<T: Ord>(set: &mut BTreeSet<T>, value: T, meter: &mut BudgetMeter) -> Result<(), Error> {
    work(meter, set.len())?;
    if !set.contains(&value) {
        meter
            .check_table_entries(set.len() as u64 + 1, &WirePath::root())
            .map_err(resource)?;
        meter
            .charge_collection_slots(1, &WirePath::root())
            .map_err(resource)?;
        work(meter, set.len())?;
        set.insert(value);
    }
    Ok(())
}
