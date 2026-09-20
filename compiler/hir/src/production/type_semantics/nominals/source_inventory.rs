use super::*;
use scoop_wire::{BudgetMeter, WirePath};

impl CanonicalSourceInheritanceInventoriesV1 {
    /// Projects public param-free source roots from a sealed HIR pair without
    /// consulting candidate interfaces. This is source data, not a checked
    /// type-section or machine-use capability.
    pub fn from_ordinary_hir(
        output: &OrdinaryHirOutput<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let nominals = roots(output, meter)?;
        inheritance::source_inventory(output.output().export.module(), &nominals, meter)
    }
}

impl CanonicalInterfaceSourceDispatchesV1 {
    /// Projects complete interface declaration order and override edges from
    /// the public roots' source inheritance closure, before schema candidates.
    pub fn from_ordinary_hir(
        output: &OrdinaryHirOutput<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let nominals = roots(output, meter)?;
        inheritance::interface_sources(output.output().export.module(), &nominals, meter)
    }
}

impl CanonicalInheritanceSourceSlotSelectionsV1 {
    /// Projects actual sealed dispatch decisions independently of candidate
    /// slot contracts, generated MIR adapters, and runtime trap functions.
    pub fn from_ordinary_hir(
        output: &OrdinaryHirOutput<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let nominals = roots(output, meter)?;
        inheritance::slot_selections(output.output().export.module(), &nominals, meter)
    }
}

impl CanonicalInheritanceSourceCallablesV1 {
    /// Projects only actual source slot roots and implementation targets from
    /// sealed HIR, including contracts outside the public lookup surface.
    pub fn from_ordinary_hir(
        output: &OrdinaryHirOutput<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let nominals = roots(output, meter)?;
        let export = output.output().export.module();
        let inventory = inheritance::source_inventory(export, &nominals, meter)?;
        let selections = inheritance::slot_selections(export, &nominals, meter)?;
        inheritance::source_callables(export, &inventory, &selections, meter)
    }
}

impl CanonicalInheritanceSourceConstructorsV1 {
    /// Projects public/protected constructor contracts directly from sealed
    /// source declarations, independently of candidate callable interfaces.
    pub fn from_ordinary_hir(
        output: &OrdinaryHirOutput<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let nominals = roots(output, meter)?;
        let export = output.output().export.module();
        let inventory = inheritance::source_inventory(export, &nominals, meter)?;
        inheritance::source_constructors(export, &nominals, &inventory, meter)
    }
}

impl CanonicalInheritanceSourceProtectedCallablesV1 {
    /// Projects protected source methods and accessors before candidate
    /// interfaces, including source-only generic method metadata.
    pub fn from_ordinary_hir(
        output: &OrdinaryHirOutput<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let nominals = roots(output, meter)?;
        let export = output.output().export.module();
        let inventory = inheritance::source_inventory(export, &nominals, meter)?;
        inheritance::source_protected_callables(export, &inventory, meter)
    }
}

impl CanonicalInheritanceSourcePropertiesV1 {
    /// Projects logical properties required by protected members and actual
    /// dispatch sources, without using public or candidate property tables.
    pub fn from_ordinary_hir(
        output: &OrdinaryHirOutput<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let nominals = roots(output, meter)?;
        let export = output.output().export.module();
        let inventory = inheritance::source_inventory(export, &nominals, meter)?;
        let selections = inheritance::slot_selections(export, &nominals, meter)?;
        inheritance::source_properties(export, &inventory, &selections, meter)
    }
}

fn roots<'a>(
    output: &'a OrdinaryHirOutput<'_>,
    meter: &mut BudgetMeter,
) -> Result<Vec<ConcreteNominal<'a>>, Error> {
    let export = output.output().export.module();
    let local = output.output().local.module();
    let mut nominals = Vec::new();
    for local_id in public_nominals(export) {
        meter.charge_work(1, &WirePath::root()).map_err(resource)?;
        let source = identity(export, local_id)?.source().ok_or_else(|| {
            let (kind, index) = location(local_id);
            Error::GeneratedPublicNominal { kind, index }
        })?;
        if let Some(nominal) = concrete(export, local, local_id, source)? {
            meter
                .check_table_entries(nominals.len() as u64 + 1, &WirePath::root())
                .map_err(resource)?;
            meter
                .try_reserve_collection_slots(&mut nominals, 1, &WirePath::root())
                .map_err(resource)?;
            nominals.push(nominal);
        }
    }
    Ok(nominals)
}

pub(super) fn concrete<'a>(
    export: &ExportHir,
    local: &LocalConcreteHir,
    local_id: NominalLocalId,
    source: &'a HirSourceNominalIdentity,
) -> Result<Option<ConcreteNominal<'a>>, Error> {
    let Some(owner) = source.concrete_id() else {
        return Ok(None);
    };
    let exact =
        PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(owner)).map_err(|error| {
            Error::InvalidSourceShape {
                declaration: source_id(source),
                reason: error.to_string(),
            }
        })?;
    verify_exact_pair(export, local, local_id, exact)?;
    Ok(Some(ConcreteNominal {
        local: local_id,
        source,
        owner,
        exact,
    }))
}

fn resource(error: scoop_wire::WireError) -> Error {
    Error::SourceInventory(SourceInventoryError::Resource(error))
}
