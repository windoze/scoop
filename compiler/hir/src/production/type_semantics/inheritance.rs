use scoop_identity::PersistentExactTypeId;

use super::CrossConeTypeSemanticsProductionError as Error;
use super::nominals::{ConcreteNominal, NominalLocalId};
use crate::*;

mod edges;
use edges::exact;
pub(super) use edges::project_edges;
mod schemas;
mod slots;
pub(super) use schemas::slot_selections;
pub(super) use slots::SlotContracts;
pub(in crate::production) mod source_errors;
pub(in crate::production::type_semantics) mod source_inventory;
pub(super) use source_inventory::project as source_inventory;

pub(super) fn produce(
    export: &ExportHir,
    nominals: &[ConcreteNominal<'_>],
    inventory: &CanonicalSourceInheritanceInventoriesV1,
    slots: &SlotContracts<'_>,
) -> Result<CanonicalNominalInheritanceInterfacesV1, Error> {
    let mut records = Vec::with_capacity(nominals.len());
    for nominal in nominals {
        let source = inventory
            .get(nominal.exact)
            .ok_or(Error::MissingLocalSupport(nominal.exact))?;
        let record = NominalInheritanceInterfaceV1::try_new(
            project_edges(export, nominal)?,
            slots.project(nominal.exact, source.slot_schemas())?,
            source.protected_members().clone(),
            source.slot_schemas().clone(),
        )
        .map_err(|error| Error::InvalidInheritance {
            exact: nominal.exact,
            reason: error.to_string(),
        })?;
        records.push(record);
    }
    CanonicalNominalInheritanceInterfacesV1::try_new(records).map_err(|error| Error::InvalidTable {
        table: "inheritance",
        reason: error.to_string(),
    })
}
