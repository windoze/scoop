use super::source_resources::{invalid, resource, work};
use super::*;
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{BudgetMeter, WirePath};
use std::collections::BTreeSet;

impl CanonicalInheritanceSourceParameterProtocolsV1 {
    /// Projects a closed Export HIR's source protocols for an independent
    /// inheritance inventory. Inventory ownership and artifact binding remain
    /// obligations of the complete source-authority transaction.
    pub fn from_export_hir(
        output: &ExportHirOutput,
        inventory: &CanonicalSourceInheritanceInventoriesV1,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        project(output.module(), inventory, meter)
    }
}

pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    inventory: &CanonicalSourceInheritanceInventoriesV1,
    meter: &mut BudgetMeter,
) -> Result<CanonicalInheritanceSourceParameterProtocolsV1, Error> {
    let mut required = BTreeSet::new();
    for owner in inventory.records() {
        work(meter, 1)?;
        for constructor in owner.constructors().values() {
            insert(
                &mut required,
                CallableTemplateOrigin::Constructor(*constructor),
                meter,
            )?;
        }
        for member in owner.protected_members().values() {
            work(meter, 1)?;
            if let ProtectedDeclarationRefV1::Callable(callable) = member {
                if matches!(
                    callable.declaration(),
                    CallableTemplateOrigin::Function(_)
                        | CallableTemplateOrigin::GenericFunction(_)
                ) {
                    insert(&mut required, callable.declaration(), meter)?;
                }
            }
        }
    }
    let sources = super::super::nominal_parameters::project(export, required, meter)?;
    let path = WirePath::root();
    let mut records = Vec::new();
    meter
        .try_reserve_collection_slots(&mut records, sources.len(), &path)
        .map_err(resource)?;
    for source in sources {
        records.push(
            InheritanceSourceParameterProtocolV1::try_from(source)
                .map_err(Error::SourceInventory)?,
        );
    }
    CanonicalInheritanceSourceParameterProtocolsV1::try_new(records, meter)
        .map_err(Error::SourceInventory)
}

fn insert(
    required: &mut BTreeSet<CallableTemplateOrigin>,
    declaration: CallableTemplateOrigin,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let path = WirePath::root();
    work(meter, required.len())?;
    meter
        .check_table_entries(required.len() as u64 + 1, &path)
        .map_err(resource)?;
    meter.charge_collection_slots(1, &path).map_err(resource)?;
    if !required.insert(declaration) {
        return Err(invalid(
            "parameter protocol belongs to multiple inheritance owners",
        ));
    }
    Ok(())
}
