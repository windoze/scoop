use super::*;
use scoop_wire::{BudgetMeter, WirePath};
use std::collections::BTreeSet;

use super::source_resources::{invalid, resource, work};

pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    inventory: &CanonicalSourceInheritanceInventoriesV1,
    meter: &mut BudgetMeter,
) -> Result<CanonicalInheritanceSourceProtectedCallablesV1, Error> {
    let mut required = BTreeSet::new();
    for owner in inventory.records() {
        work(meter, 1)?;
        for member in owner.protected_members().values() {
            work(meter, required.len())?;
            if let ProtectedDeclarationRefV1::Callable(declaration) = member {
                meter
                    .charge_collection_slots(1, &WirePath::root())
                    .map_err(resource)?;
                meter
                    .check_table_entries(required.len() as u64 + 1, &WirePath::root())
                    .map_err(resource)?;
                if !required.insert(declaration.declaration()) {
                    return Err(invalid(
                        "protected callable belongs to more than one source owner",
                    ));
                }
            }
        }
    }
    let sources = super::super::nominal_callables::project(export, required, meter)?;
    let mut records = Vec::new();
    meter
        .try_reserve_collection_slots(&mut records, sources.len(), &WirePath::root())
        .map_err(resource)?;
    for source in sources {
        work(meter, 1)?;
        records.push(ProtectedCallableInterfaceV1::try_from(source).map_err(invalid)?);
    }
    CanonicalInheritanceSourceProtectedCallablesV1::try_new(records, meter)
        .map_err(Error::SourceInventory)
}
