use super::*;
use scoop_wire::{BudgetMeter, WirePath};

impl CanonicalSourceInheritanceInventoriesV1 {
    /// Projects the local param-free source closure from a sealed HIR pair without
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
    /// the required source inheritance closure, before schema candidates.
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

impl CanonicalInheritanceSourceParameterProtocolsV1 {
    /// Projects source argument facts for the independent inheritance inventory,
    /// without deriving names, calling categories or origins from candidates.
    pub fn from_ordinary_hir(
        output: &OrdinaryHirOutput<'_>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let nominals = roots(output, meter)?;
        let export = output.output().export.module();
        let inventory = inheritance::source_inventory(export, &nominals, meter)?;
        inheritance::source_parameters(export, &inventory, meter)
    }
}

fn roots<'a>(
    output: &'a OrdinaryHirOutput<'_>,
    meter: &mut BudgetMeter,
) -> Result<Vec<ConcreteNominal<'a>>, Error> {
    let required = CanonicalSourceNominalIdsV1::from_export_hir(&output.output().export, meter)?;
    from_required(output, &required, meter)
}

pub(super) fn from_required<'a>(
    output: &'a OrdinaryHirOutput<'_>,
    required: &CanonicalSourceNominalIdsV1,
    meter: &mut BudgetMeter,
) -> Result<Vec<ConcreteNominal<'a>>, Error> {
    let export = output.output().export.module();
    let local = output.output().local.module();
    let mut nominals = Vec::new();
    let mut found = 0;
    for local_id in authority_projection::all_nominals(export) {
        inheritance::source_resources::work(meter, required.values().len())?;
        let Some(source) = identity(export, local_id)?.source() else {
            continue;
        };
        if source.declaration().origin() != export.cone
            || required.values().binary_search(&source_id(source)).is_err()
        {
            continue;
        }
        found += 1;
        if let Some(nominal) = concrete(export, local, local_id, source, meter)? {
            meter
                .check_table_entries(nominals.len() as u64 + 1, &WirePath::root())
                .map_err(resource)?;
            meter
                .try_reserve_collection_slots(&mut nominals, 1, &WirePath::root())
                .map_err(resource)?;
            nominals.push(nominal);
        }
    }
    if found != required.values().len() {
        return Err(inheritance::source_resources::invalid(
            "required concrete source owner is absent from sealed HIR",
        ));
    }
    Ok(nominals)
}

fn concrete<'a>(
    export: &ExportHir,
    local: &LocalConcreteHir,
    local_id: NominalLocalId,
    source: &'a HirSourceNominalIdentity,
    meter: &mut BudgetMeter,
) -> Result<Option<ConcreteNominal<'a>>, Error> {
    let Some(owner) = source.concrete_id() else {
        return Ok(None);
    };
    let key = ExactTypeKey::Nominal(owner);
    let length =
        scoop_wire::encoded_length(&key).map_err(inheritance::source_resources::invalid)?;
    meter
        .charge_sha256(length, &WirePath::root())
        .map_err(resource)?;
    let exact =
        PersistentExactTypeId::from_key(&key).map_err(|error| Error::InvalidSourceShape {
            declaration: source_id(source),
            reason: error.to_string(),
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
