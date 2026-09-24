use std::collections::BTreeSet;

use scoop_identity::{PersistentDispatchSlotId, PersistentExactTypeId};
use scoop_wire::{BudgetMeter, WirePath};

use super::{ConcreteNominal, Error, NominalLocalId, exact};
use crate::*;

mod classes;
mod interfaces;
mod selections;
mod source;
pub(in crate::production::type_semantics) use selections::project as slot_selections;
pub(in crate::production::type_semantics) use source::project as interface_sources;

pub(super) fn project(
    export: &ExportHir,
    nominal: &ConcreteNominal<'_>,
    meter: &mut BudgetMeter,
) -> Result<CanonicalInheritanceSlotSchemasV1, Error> {
    let mut projection = Projection {
        export,
        owner: nominal.exact,
        meter,
    };
    let mut schemas = Vec::new();
    let mut interfaces = Vec::new();
    match nominal.local {
        NominalLocalId::Class(id) => {
            let (vtable, implemented) = projection.class(id)?;
            projection.push(&mut schemas, vtable)?;
            interfaces = implemented;
        }
        NominalLocalId::Object(id) => {
            let (vtable, implemented) = projection.class(export.objects[id].backing_class)?;
            projection.push(&mut schemas, vtable)?;
            interfaces = implemented;
        }
        NominalLocalId::Interface(id) => {
            let schema = projection.interface(export.interfaces[id].self_application)?;
            projection.push(&mut schemas, schema)?;
        }
        NominalLocalId::Struct(id) => {
            projection.extend(&mut interfaces, &export.structs[id].interfaces)?;
        }
        NominalLocalId::Enum(id) => {
            projection.extend(&mut interfaces, &export.enums[id].interfaces)?;
        }
    }
    let mut seen = BTreeSet::new();
    for ty in interfaces {
        let application = projection.interface_application(ty)?;
        for inherited in projection.interface_postorder(application)? {
            projection.search(seen.len())?;
            if seen.insert(inherited) {
                let schema = projection.interface(inherited)?;
                projection.push(&mut schemas, schema)?;
            }
        }
    }
    projection.sort_work(schemas.len())?;
    CanonicalInheritanceSlotSchemasV1::try_new(schemas).map_err(|error| projection.invalid(error))
}

struct Projection<'a, 'm> {
    export: &'a ExportHir,
    owner: PersistentExactTypeId,
    meter: &'m mut BudgetMeter,
}

impl Projection<'_, '_> {
    fn invalid(&self, reason: impl std::fmt::Display) -> Error {
        Error::InvalidInheritance {
            exact: self.owner,
            reason: reason.to_string(),
        }
    }

    fn work(&mut self, count: usize) -> Result<(), Error> {
        self.meter
            .charge_work(count as u64, &WirePath::root())
            .map_err(resource)
    }

    fn search(&mut self, length: usize) -> Result<(), Error> {
        self.work(length.max(1).ilog2() as usize + 1)?;
        self.meter
            .charge_collection_slots(1, &WirePath::root())
            .map_err(resource)
    }

    fn sort_work(&mut self, length: usize) -> Result<(), Error> {
        for _ in 0..=length.max(1).ilog2() {
            self.work(length)?;
        }
        Ok(())
    }

    fn reserve<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Result<(), Error> {
        self.meter
            .check_table_entries(
                values.len().saturating_add(additional) as u64,
                &WirePath::root(),
            )
            .map_err(resource)?;
        self.meter
            .try_reserve_collection_slots(values, additional, &WirePath::root())
            .map_err(resource)
    }

    fn push<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), Error> {
        self.reserve(values, 1)?;
        values.push(value);
        Ok(())
    }

    fn extend<T: Copy>(&mut self, values: &mut Vec<T>, additions: &[T]) -> Result<(), Error> {
        self.work(additions.len())?;
        self.reserve(values, additions.len())?;
        values.extend_from_slice(additions);
        Ok(())
    }

    fn schema(
        &mut self,
        role: InheritanceSlotSchemaRoleV1,
        slots: Vec<PersistentDispatchSlotId>,
    ) -> Result<InheritanceSlotSchemaV1, Error> {
        self.sort_work(slots.len())?;
        self.meter
            .charge_collection_slots(slots.len() as u64, &WirePath::root())
            .map_err(resource)?;
        InheritanceSlotSchemaV1::try_new(role, slots).map_err(|error| self.invalid(error))
    }
}

fn resource(error: scoop_wire::WireError) -> Error {
    Error::SourceInventory(SourceInventoryError::Resource(error))
}
