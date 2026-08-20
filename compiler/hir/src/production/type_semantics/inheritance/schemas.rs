use std::collections::BTreeSet;

use scoop_identity::{PersistentDispatchSlotId, PersistentExactTypeId};
use scoop_wire::WirePath;

use super::{ConcreteNominal, Error, NominalLocalId, exact};
use crate::*;

mod classes;
mod interfaces;
mod selections;
pub(in crate::production::type_semantics) use selections::project as slot_selections;

pub(super) fn project(
    export: &ExportHir,
    nominal: &ConcreteNominal<'_>,
) -> Result<CanonicalInheritanceSlotSchemasV1, Error> {
    let mut projection = Projection {
        export,
        owner: nominal.exact,
    };
    let mut schemas = Vec::new();
    let conformances = match nominal.local {
        NominalLocalId::Class(id) => {
            let vtable = projection.class(id)?;
            projection.push(&mut schemas, vtable)?;
            &export.classes[id].interface_implementations
        }
        NominalLocalId::Object(id) => {
            let class = export.objects[id].backing_class;
            let vtable = projection.class(class)?;
            projection.push(&mut schemas, vtable)?;
            &export.classes[class].interface_implementations
        }
        NominalLocalId::Interface(id) => {
            let schema = projection.interface(export.interfaces[id].self_application)?;
            projection.push(&mut schemas, schema)?;
            return CanonicalInheritanceSlotSchemasV1::try_new(schemas)
                .map_err(|error| projection.invalid(error));
        }
        NominalLocalId::Struct(id) => &export.structs[id].interface_implementations,
        NominalLocalId::Enum(id) => &export.enums[id].interface_implementations,
    };
    for conformance in conformances {
        let interface_exact = exact(export, conformance.interface)?;
        let slots = conformance
            .methods
            .iter()
            .map(|method| projection.interface_slot(method.member))
            .collect::<Result<Vec<_>, Error>>()?;
        let schema = projection.schema(
            InheritanceSlotSchemaRoleV1::Interface { interface_exact },
            slots,
        )?;
        projection.push(&mut schemas, schema)?;
    }

    CanonicalInheritanceSlotSchemasV1::try_new(schemas).map_err(|error| projection.invalid(error))
}

struct Projection<'a> {
    export: &'a ExportHir,
    owner: PersistentExactTypeId,
}

impl Projection<'_> {
    fn invalid(&self, reason: impl std::fmt::Display) -> Error {
        Error::InvalidInheritance {
            exact: self.owner,
            reason: reason.to_string(),
        }
    }

    fn reserve<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Result<(), Error> {
        scoop_wire::allocation::try_reserve(values, additional, &WirePath::root()).map_err(resource)
    }

    fn push<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), Error> {
        self.reserve(values, 1)?;
        values.push(value);
        Ok(())
    }

    fn schema(
        &mut self,
        role: InheritanceSlotSchemaRoleV1,
        slots: Vec<PersistentDispatchSlotId>,
    ) -> Result<InheritanceSlotSchemaV1, Error> {
        InheritanceSlotSchemaV1::try_new(role, slots).map_err(|error| self.invalid(error))
    }
}

fn resource(error: scoop_wire::WireError) -> Error {
    Error::SourceInventory(SourceInventoryError::Resource(error))
}
