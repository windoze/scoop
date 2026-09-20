use super::*;
use scoop_wire::{BudgetMeter, WirePath};
use std::collections::BTreeMap;
use std::collections::BTreeSet;

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
    meter
        .charge_collection_slots(required.len() as u64, &path)
        .map_err(resource)?;
    meter
        .charge_work(required.len() as u64, &path)
        .map_err(resource)?;
    let ids: BTreeSet<_> = required.keys().copied().collect();
    let records = super::super::nominal_constructors::project(export, ids, meter)?;
    for record in &records {
        work(meter, required.len())?;
        let nominal = required
            .get(&record.declaration())
            .ok_or_else(|| invalid("constructor is absent from inheritance inventory"))?;
        if record.payload().owner() != SourceNominalId::Concrete(nominal.owner)
            || !matches!(
                record.declaration_access().declared_visibility(),
                DeclaredVisibilityV1::Public | DeclaredVisibilityV1::Protected
            )
        {
            return Err(Error::MissingConstructor(nominal.exact));
        }
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
