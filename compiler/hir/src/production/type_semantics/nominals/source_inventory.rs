use super::*;
use scoop_wire::WirePath;

impl CanonicalSourceInheritanceInventoriesV1 {
    /// Projects the closed local representation/inheritance subset from sealed
    /// declarations. Full source contracts remain available for excluded roots;
    /// this inventory itself grants no machine-use capability.
    pub fn from_dependency_hir(output: &DependencyHirOutput) -> Result<Self, Error> {
        let nominals = roots(output)?;
        inheritance::source_inventory(output.output().export.module(), &nominals)
    }
}

impl CanonicalInterfaceSourceDispatchesV1 {
    /// Projects complete interface declaration order and override edges from
    /// the required source inheritance closure, before schema candidates.
    pub fn from_dependency_hir(output: &DependencyHirOutput) -> Result<Self, Error> {
        let nominals = roots(output)?;
        inheritance::interface_sources(output.output().export.module(), &nominals)
    }
}

impl CanonicalInheritanceSourceSlotSelectionsV1 {
    /// Projects actual sealed dispatch decisions independently of candidate
    /// slot contracts, generated MIR adapters, and runtime trap functions.
    pub fn from_dependency_hir(output: &DependencyHirOutput) -> Result<Self, Error> {
        let nominals = roots(output)?;
        inheritance::slot_selections(output.output().export.module(), &nominals)
    }
}

impl CanonicalInheritanceSourceCallablesV1 {
    /// Projects only actual source slot roots and implementation targets from
    /// sealed HIR, including contracts outside the public lookup surface.
    pub fn from_dependency_hir(output: &DependencyHirOutput) -> Result<Self, Error> {
        let nominals = roots(output)?;
        let export = output.output().export.module();
        let inventory = inheritance::source_inventory(export, &nominals)?;
        let selections = inheritance::slot_selections(export, &nominals)?;
        inheritance::source_callables(export, &inventory, &selections)
    }
}

impl CanonicalInheritanceSourceConstructorsV1 {
    /// Projects public/protected constructor contracts directly from sealed
    /// source declarations, independently of candidate callable interfaces.
    pub fn from_dependency_hir(output: &DependencyHirOutput) -> Result<Self, Error> {
        let nominals = roots(output)?;
        let export = output.output().export.module();
        let inventory = inheritance::source_inventory(export, &nominals)?;
        inheritance::source_constructors(export, &nominals, &inventory)
    }
}

impl CanonicalInheritanceSourceProtectedCallablesV1 {
    /// Projects protected source methods and accessors before candidate
    /// interfaces, including source-only generic method metadata.
    pub fn from_dependency_hir(output: &DependencyHirOutput) -> Result<Self, Error> {
        let nominals = roots(output)?;
        let export = output.output().export.module();
        let inventory = inheritance::source_inventory(export, &nominals)?;
        inheritance::source_protected_callables(export, &inventory)
    }
}

impl CanonicalInheritanceSourcePropertiesV1 {
    /// Projects logical properties required by protected members and actual
    /// dispatch sources, without using public or candidate property tables.
    pub fn from_dependency_hir(output: &DependencyHirOutput) -> Result<Self, Error> {
        let nominals = roots(output)?;
        let export = output.output().export.module();
        let inventory = inheritance::source_inventory(export, &nominals)?;
        let selections = inheritance::slot_selections(export, &nominals)?;
        inheritance::source_properties(export, &inventory, &selections)
    }
}

impl CanonicalInheritanceSourceParameterProtocolsV1 {
    /// Projects source argument facts for the independent inheritance inventory,
    /// without deriving names, calling categories or origins from candidates.
    pub fn from_dependency_hir(output: &DependencyHirOutput) -> Result<Self, Error> {
        let nominals = roots(output)?;
        let export = output.output().export.module();
        let inventory = inheritance::source_inventory(export, &nominals)?;
        inheritance::source_parameters(export, &inventory)
    }
}

fn roots<'a>(output: &'a DependencyHirOutput) -> Result<Vec<ConcreteNominal<'a>>, Error> {
    let required = CanonicalSourceNominalIdsV1::from_export_hir(&output.output().export)?;
    from_required(output, &required)
}

pub(super) fn from_required<'a>(
    output: &'a DependencyHirOutput,
    required: &CanonicalSourceNominalIdsV1,
) -> Result<Vec<ConcreteNominal<'a>>, Error> {
    from_pair(output.output(), required)
}

pub(super) fn from_pair<'a>(
    output: &'a Output,
    required: &CanonicalSourceNominalIdsV1,
) -> Result<Vec<ConcreteNominal<'a>>, Error> {
    let export = output.export.module();
    let local = output.local.module();
    let materialization = materialization::closure(output)?;
    let mut nominals = Vec::new();
    let mut found = 0;
    for local_id in authority_projection::all_nominals(export) {
        let Some(source) = identity(export, local_id)?.source() else {
            continue;
        };
        if source.declaration().origin() != export.cone
            || required.values().binary_search(&source_id(source)).is_err()
        {
            continue;
        }
        found += 1;
        if let Some(owner) = source.concrete_id() {
            if !materialization.contains(owner) {
                continue;
            }
        }
        if let Some(nominal) = concrete(export, local, local_id, source)? {
            scoop_wire::allocation::try_reserve(&mut nominals, 1, &WirePath::root())
                .map_err(resource)?;
            nominals.push(nominal);
        }
    }
    if found != required.values().len() {
        return Err(inheritance::source_errors::invalid(
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
) -> Result<Option<ConcreteNominal<'a>>, Error> {
    let Some(owner) = source.concrete_id() else {
        return Ok(None);
    };
    let key = ExactTypeKey::Nominal(owner);

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
