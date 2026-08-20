//! Completes source invoke keys without materializing generated callable bodies.
use super::*;
use std::collections::btree_map::Entry;

pub(super) fn complete(
    export: &ExportHir,
    foundation: &mut CanonicalHirFoundation,
) -> Result<(), HirFoundationBuildError> {
    use HirFoundationBuildError as Error;

    let mut keys: BTreeMap<_, _> = std::mem::take(&mut foundation.generated_callables)
        .into_iter()
        .map(|r| (r.id(), r))
        .collect();
    let mut origins: BTreeMap<_, _> = std::mem::take(&mut foundation.definition_origins)
        .into_iter()
        .map(|r| (r.subject(), r))
        .collect();
    crate::production::visit_source_callable_reference_keys(export, &mut |record, origin| {
        match keys.entry(record.id()) {
            Entry::Vacant(entry) => {
                entry.insert(record);
            }
            Entry::Occupied(entry) => {
                if entry.get() != &record {
                    return Err(Error::DuplicateIdentity {
                        table: HirFoundationTable::GeneratedCallable,
                        identity: *entry.key().as_array(),
                    });
                }
            }
        }
        match origins.entry(origin.subject()) {
            Entry::Vacant(entry) => {
                entry.insert(origin);
            }
            Entry::Occupied(entry) => {
                if entry.get() != &origin {
                    return Err(Error::DuplicateSubject {
                        table: HirFoundationTable::DefinitionOrigin,
                        subject_tag: entry.key().kind_tag(),
                        subject: entry.key().raw_id(),
                    });
                }
            }
        }
        Ok(())
    })?;

    foundation.set_generated_callables(keys.into_values().collect())?;
    foundation.set_definition_origins(origins.into_values().collect())
}
