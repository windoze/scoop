//! Project dispatch relationships already resolved in the sealed source HIR.

use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::PersistentDispatchSlotId;
use scoop_wire::{BudgetMeter, WirePath};

use super::type_semantics::inheritance::source_resources::{invalid, resource};
use crate::{CrossConeTypeSemanticsProductionError as Error, *};

mod interfaces;
mod selections;
mod targets;

type Selection = InheritanceSourceSlotSelectionV1;
type Selections = BTreeMap<PersistentDispatchSlotId, Selection>;

pub(super) fn project(
    export: &ExportHir,
    owner: NominalOwner,
    meter: &mut BudgetMeter,
) -> Result<CanonicalNominalDispatchSelectionsV1, Error> {
    let mut projection = Projection::new(export, meter);
    let mut records = Vec::new();
    for (slot, selection) in projection.selections(owner)? {
        projection.push(
            &mut records,
            NominalDispatchSelectionV1::new(slot, selection),
        )?;
    }
    CanonicalNominalDispatchSelectionsV1::try_new(records, meter).map_err(|error| match error {
        NominalDispatchSelectionError::Resource(error) => resource(error),
        error => invalid(error),
    })
}

pub(super) struct Projection<'a, 'm> {
    export: &'a ExportHir,
    meter: &'m mut BudgetMeter,
}

impl<'a, 'm> Projection<'a, 'm> {
    pub(super) fn new(export: &'a ExportHir, meter: &'m mut BudgetMeter) -> Self {
        Self { export, meter }
    }

    fn work(&mut self, count: usize) -> Result<(), Error> {
        self.meter
            .charge_work(count as u64, &WirePath::root())
            .map_err(resource)
    }

    fn search(&mut self, count: usize) -> Result<(), Error> {
        self.work(count.max(1).ilog2() as usize + 1)?;
        self.meter
            .charge_collection_slots(1, &WirePath::root())
            .map_err(resource)
    }

    fn depth(&mut self, depth: usize) -> Result<(), Error> {
        self.meter
            .check_semantic_depth(depth as u64, &WirePath::root())
            .map_err(resource)
    }

    fn push<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), Error> {
        self.meter
            .check_table_entries(values.len() as u64 + 1, &WirePath::root())
            .map_err(resource)?;
        self.meter
            .try_reserve_collection_slots(values, 1, &WirePath::root())
            .map_err(resource)?;
        values.push(value);
        Ok(())
    }

    pub(super) fn class_chain(&mut self, class: ClassId) -> Result<Vec<ClassId>, Error> {
        let mut result = Vec::new();
        let mut seen = BTreeSet::new();
        let mut current = class;
        loop {
            self.depth(result.len() + 1)?;
            self.search(seen.len())?;
            if !seen.insert(current) {
                return Err(invalid("cycle in the source class base chain"));
            }
            self.push(&mut result, current)?;
            let Some(base) = self.export.classes[current].base_class else {
                return Ok(result);
            };
            let Type::Class(application) = self.export.types[base] else {
                return Err(invalid(
                    "class base does not resolve to a class application",
                ));
            };
            current = self.export.class_applications[application].template;
        }
    }
}
