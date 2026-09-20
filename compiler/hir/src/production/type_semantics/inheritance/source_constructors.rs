use super::*;
use scoop_identity::{PersistentConstructorId, SourceDeclarationKey};
use scoop_wire::{BudgetMeter, WirePath};
use std::collections::BTreeMap;

mod contract;

struct Constructor<'a> {
    declaration: PersistentConstructorId,
    key: &'a SourceDeclarationKey,
    parameters: ExportParameterOwner,
    result: TypeId,
    visibility: DeclaredVisibility,
}

pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    nominals: &[ConcreteNominal<'_>],
    inventory: &CanonicalSourceInheritanceInventoriesV1,
    meter: &mut BudgetMeter,
) -> Result<CanonicalInheritanceSourceConstructorsV1, Error> {
    let path = WirePath::root();
    let mut required = BTreeMap::new();
    for nominal in nominals {
        work(meter, inventory.records().len())?;
        let source = inventory
            .get(nominal.exact)
            .ok_or(Error::MissingConstructor(nominal.exact))?;
        for id in source.constructors().values() {
            work(meter, required.len())?;
            meter
                .check_table_entries(required.len() as u64 + 1, &path)
                .map_err(resource)?;
            meter.charge_collection_slots(1, &path).map_err(resource)?;
            if required.insert(*id, nominal).is_some() {
                return Err(invalid("constructor belongs to multiple source owners"));
            }
        }
    }
    let mut records = Vec::new();
    meter
        .try_reserve_collection_slots(&mut records, required.len(), &path)
        .map_err(resource)?;
    for (id, source) in export.struct_constructors.iter() {
        work(meter, required.len())?;
        let identity = &export.constructor_identities[id];
        if let Some(nominal) = required.remove(&identity.id()) {
            let owner = &export.structs[source.owner];
            records.push(contract::project(
                export,
                nominal,
                Constructor {
                    declaration: identity.id(),
                    key: identity.key(),
                    parameters: ExportParameterOwner::StructConstructor(id),
                    result: export.struct_applications[owner.self_application].canonical_type,
                    visibility: source.access.declared,
                },
                meter,
            )?);
        }
    }
    for (id, source) in export.class_constructors.iter() {
        work(meter, required.len())?;
        let Some(identity) = export.constructor_identities[id].source_record() else {
            continue;
        };
        if let Some(nominal) = required.remove(&identity.id()) {
            let owner = &export.classes[source.owner];
            records.push(contract::project(
                export,
                nominal,
                Constructor {
                    declaration: identity.id(),
                    key: identity.key(),
                    parameters: ExportParameterOwner::ClassConstructor(id),
                    result: export.class_applications[owner.self_application].canonical_type,
                    visibility: source.access.declared,
                },
                meter,
            )?);
        }
    }
    if !required.is_empty() {
        return Err(invalid(
            "required constructor has no sealed source declaration",
        ));
    }
    CanonicalInheritanceSourceConstructorsV1::try_new(records, meter)
        .map_err(Error::SourceInventory)
}

fn work(meter: &mut BudgetMeter, length: usize) -> Result<(), Error> {
    meter
        .charge_work(u64::from(length.max(1).ilog2()) + 1, &WirePath::root())
        .map_err(resource)
}

fn resource(error: scoop_wire::WireError) -> Error {
    Error::SourceInventory(SourceInventoryError::Resource(error))
}

fn invalid(reason: impl ToString) -> Error {
    Error::InvalidSourceDeclaration(reason.to_string())
}
