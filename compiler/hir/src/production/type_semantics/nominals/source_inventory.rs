use super::*;
use scoop_wire::WirePath;

pub(super) fn from_required<'a>(
    output: &'a DependencyHirOutput,
    required: &CanonicalSourceNominalIdsV1,
    materialization: &NominalMaterializationClosure,
) -> Result<Vec<ConcreteNominal<'a>>, Error> {
    let export = output.output().export.module();
    let local = output.output().local.module();
    let mut nominals = Vec::new();
    let mut found = 0;
    for local_id in super::all_nominals(export) {
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
