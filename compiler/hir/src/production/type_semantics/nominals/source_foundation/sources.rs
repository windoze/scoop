use super::*;
use crate::production::type_semantics::inheritance::source_resources::{
    self as resources, invalid, resource, work,
};
use scoop_identity::{DeclarationName, DeclarationScope, DuplicateSignatureKey};
use scoop_wire::WirePath;

pub(super) fn project(
    export: &ExportHir,
    required: &CanonicalSourceNominalIdsV1,
    meter: &mut BudgetMeter,
) -> Result<BTreeMap<SourceNominalId, TypeSemanticsSourceEvidenceV1>, Error> {
    let mut sources = BTreeMap::new();
    let path = WirePath::root();
    for local in authority_projection::all_nominals(export) {
        work(meter, required.values().len())?;
        let Some(source) = identity(export, local)?.source() else {
            continue;
        };
        let owner = source_id(source);
        let key = source.declaration();
        if key.origin() != export.cone || required.values().binary_search(&owner).is_err() {
            continue;
        }
        charge_key(key, meter)?;
        let subject = match owner {
            SourceNominalId::Concrete(id) => DefinitionOriginSubject::Type(id),
            SourceNominalId::GenericTemplate(id) => DefinitionOriginSubject::GenericType(id),
        };
        work(meter, export.export_definition_origins.records().len())?;
        let origin = export
            .export_definition_origins
            .get(subject)
            .ok_or(Error::MissingDefinitionOrigin(subject))?;
        resources::name(origin.origin().source().logical_path().as_str(), meter)?;
        meter
            .check_table_entries(sources.len() as u64 + 1, &path)
            .map_err(resource)?;
        meter.charge_nodes(1, &path).map_err(resource)?;
        meter.charge_collection_slots(1, &path).map_err(resource)?;
        work(meter, sources.len())?;
        work(meter, export.export_definition_origins.records().len())?;
        let evidence = TypeSemanticsSourceEvidenceV1 {
            key: key.clone(),
            access: declaration_access_for_subject(
                export,
                key,
                subject,
                nominal_access(export, local).declared.into(),
            )?,
        };
        if sources.insert(owner, evidence).is_some() {
            return Err(invalid("duplicate nominal foundation source"));
        }
    }
    if sources.len() != required.values().len() {
        return Err(invalid("required nominal foundation source is absent"));
    }
    Ok(sources)
}

fn charge_key(key: &SourceDeclarationKey, meter: &mut BudgetMeter) -> Result<(), Error> {
    let DeclarationName::Named(name) = key.name() else {
        return Err(invalid("nominal foundation source has no nominal name"));
    };
    if !matches!(
        key.duplicate_signature(),
        DuplicateSignatureKey::Nominal { .. }
    ) {
        return Err(invalid(
            "nominal foundation source has a non-nominal signature",
        ));
    }
    let path = WirePath::root();
    let owners = key.owners().owners().len() as u64;
    let packages = key.package().segments().len() as u64;
    meter
        .check_semantic_depth(owners + 1, &path)
        .map_err(resource)?;
    meter
        .check_table_entries(packages, &path)
        .map_err(resource)?;
    // The source key and the access snapshot each own the lexical owner chain.
    meter
        .charge_collection_slots(owners.saturating_mul(2).saturating_add(packages), &path)
        .map_err(resource)?;
    meter
        .charge_work(owners + packages, &path)
        .map_err(resource)?;
    resources::name(name.as_str(), meter)?;
    for segment in key.package().segments() {
        resources::name(segment.as_str(), meter)?;
    }
    if let Some(source) = key.scope().source() {
        resources::name(source.logical_path().as_str(), meter)?;
    }
    if let DeclarationScope::LexicalScoped { path: scope, .. } = key.scope() {
        let length = scope.segments().len() as u64;
        meter
            .check_semantic_depth(length + 1, &path)
            .map_err(resource)?;
        meter
            .charge_collection_slots(length, &path)
            .map_err(resource)?;
        meter.charge_work(length, &path).map_err(resource)?;
    }
    Ok(())
}
