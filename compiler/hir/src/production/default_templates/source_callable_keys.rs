//! Definition identities exist before concrete callable-reference materialization.
use super::*;
use crate::{HirFoundationBuildError as Error, HirFoundationTable};
use scoop_identity::{
    CborIdentityRecord, DefinitionOriginRecord, DefinitionOriginSubject, GeneratedCallableKey,
    PersistentGeneratedCallableId,
};

pub(crate) fn visit_source_callable_reference_keys(
    export: &ExportHir,

    visitor: &mut impl FnMut(
        CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>,
        DefinitionOriginRecord,
    ) -> Result<(), Error>,
) -> Result<(), Error> {
    let entities = DefaultEntityProjector::new(export, None);

    for (_, reference) in export.callable_references.iter() {
        let key = entities
            .callable_reference_key(reference.definition_root, &reference.definition_path)
            .map_err(|e| Error::SourceCallableReference(Box::new(e)))?;

        let record = CborIdentityRecord::from_key(key).map_err(|e| Error::IdentityDerivation {
            table: HirFoundationTable::GeneratedCallable,
            reason: e.to_string(),
        })?;
        let source = crate::production::project_definition_source(export, reference.origin)
            .map_err(Error::SourceParameterOrigin)?;
        let origin = DefinitionOriginRecord::new(
            DefinitionOriginSubject::GeneratedCallable(record.id()),
            source.origin().clone(),
        );

        visitor(record, origin)?;
    }
    Ok(())
}
