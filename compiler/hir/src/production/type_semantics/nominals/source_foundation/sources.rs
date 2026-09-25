use super::*;
use crate::production::type_semantics::inheritance::source_errors::invalid;

pub(super) fn project(
    export: &ExportHir,
    required: &CanonicalSourceNominalIdsV1,
) -> Result<BTreeMap<SourceNominalId, TypeSemanticsSourceEvidenceV1>, Error> {
    let mut sources = BTreeMap::new();

    for local in authority_projection::all_nominals(export) {
        let Some(source) = identity(export, local)?.source() else {
            continue;
        };
        let owner = source_id(source);
        let key = source.declaration();
        if key.origin() != export.cone || required.values().binary_search(&owner).is_err() {
            continue;
        }

        let subject = match owner {
            SourceNominalId::Concrete(id) => DefinitionOriginSubject::Type(id),
            SourceNominalId::GenericTemplate(id) => DefinitionOriginSubject::GenericType(id),
        };

        export
            .export_definition_origins
            .get(subject)
            .ok_or(Error::MissingDefinitionOrigin(subject))?;

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
