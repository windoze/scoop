use super::*;
use scoop_wire::WirePath;
use std::collections::BTreeSet;

use super::source_errors::{invalid, resource};

pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    inventory: &CanonicalSourceInheritanceInventoriesV1,
) -> Result<CanonicalInheritanceSourceProtectedCallablesV1, Error> {
    let mut required = BTreeSet::new();
    for owner in inventory.records() {
        for member in owner.protected_members().values() {
            if let ProtectedDeclarationRefV1::Callable(declaration) = member {
                if !required.insert(declaration.declaration()) {
                    return Err(invalid(
                        "protected callable belongs to more than one source owner",
                    ));
                }
            }
        }
    }
    let sources = super::super::nominal_callables::project(export, required)?;
    let mut records = Vec::new();
    scoop_wire::allocation::try_reserve(&mut records, sources.len(), &WirePath::root())
        .map_err(resource)?;
    for source in sources {
        records.push(ProtectedCallableInterfaceV1::try_from(source).map_err(invalid)?);
    }
    CanonicalInheritanceSourceProtectedCallablesV1::try_new(records).map_err(Error::SourceInventory)
}
