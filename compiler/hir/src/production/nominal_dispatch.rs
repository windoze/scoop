//! Project dispatch relationships already resolved in the sealed source HIR.

use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::PersistentDispatchSlotId;
use scoop_wire::WirePath;

use super::type_semantics::inheritance::source_errors::{invalid, resource};
use crate::{CrossConeTypeSemanticsProductionError as Error, *};

mod interfaces;
mod selections;
mod targets;

type Selection = InheritanceSourceSlotSelectionV1;
type Selections = BTreeMap<PersistentDispatchSlotId, Selection>;

pub(super) fn project(
    export: &ExportHir,
    owner: NominalOwner,
) -> Result<CanonicalNominalDispatchSelectionsV1, Error> {
    let mut projection = Projection::new(export);
    let mut records = Vec::new();
    for (slot, selection) in projection.selections(owner)? {
        projection.push(
            &mut records,
            NominalDispatchSelectionV1::new(slot, selection),
        )?;
    }
    CanonicalNominalDispatchSelectionsV1::try_new(records).map_err(|error| match error {
        NominalDispatchSelectionError::Resource(error) => resource(error),
        error => invalid(error),
    })
}

pub(super) struct Projection<'a> {
    export: &'a ExportHir,
}

impl<'a> Projection<'a> {
    pub(super) fn new(export: &'a ExportHir) -> Self {
        Self { export }
    }

    fn push<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), Error> {
        scoop_wire::allocation::try_reserve(values, 1, &WirePath::root()).map_err(resource)?;
        values.push(value);
        Ok(())
    }

    pub(super) fn class_chain(&mut self, class: ClassId) -> Result<Vec<ClassId>, Error> {
        let mut result = Vec::new();
        let mut seen = BTreeSet::new();
        let mut current = class;
        loop {
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
