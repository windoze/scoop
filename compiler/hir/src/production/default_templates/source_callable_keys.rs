//! Definition identities exist before concrete callable-reference materialization.
use super::*;
use crate::{HirFoundationBuildError as Error, HirFoundationTable};
use scoop_identity::{
    CborIdentityRecord, DefinitionOriginRecord, DefinitionOriginSubject, GeneratedCallableKey,
    PersistentGeneratedCallableId,
};
use scoop_wire::{BudgetMeter, WirePath};

pub(crate) fn visit_source_callable_reference_keys(
    export: &ExportHir,
    meter: &mut BudgetMeter,
    visitor: &mut impl FnMut(
        CborIdentityRecord<PersistentGeneratedCallableId, GeneratedCallableKey>,
        DefinitionOriginRecord,
        &mut BudgetMeter,
    ) -> Result<(), Error>,
) -> Result<(), Error> {
    let entities = DefaultEntityProjector::new(export, None, meter);
    entities
        .resources
        .with_meter(|meter, _| {
            let path = WirePath::root();
            meter.check_table_entries(export.callable_references.len() as u64, &path)?;
            meter.charge_work(export.callable_references.len() as u64, &path)
        })
        .map_err(Error::DefaultSourceResource)?;
    for (_, reference) in export.callable_references.iter() {
        let key = entities
            .callable_reference_key(reference.definition_root, &reference.definition_path)
            .map_err(|e| Error::SourceCallableReference(Box::new(e)))?;
        entities
            .charge_origin(reference.origin)
            .map_err(Error::DefaultSourceResource)?;
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
        entities
            .resources
            .with_meter(|meter, _| {
                meter.charge_nodes(1, &WirePath::root())?;
                Ok::<_, scoop_wire::WireError>(())
            })
            .map_err(Error::DefaultSourceResource)?;
        // The callback receives the same transaction meter; no side budget is created.
        entities
            .resources
            .with_fallible_meter(|meter, _| visitor(record, origin, meter))?;
    }
    Ok(())
}
