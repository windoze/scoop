//! Completes source invoke keys without materializing generated callable bodies.
use super::*;
use scoop_wire::WirePath;
use std::collections::btree_map::Entry;

pub(super) fn complete(
    export: &ExportHir,
    foundation: &mut CanonicalHirFoundation,
    meter: &mut BudgetMeter,
) -> Result<(), HirFoundationBuildError> {
    use HirFoundationBuildError as Error;
    let path = WirePath::root();
    charge_tables(
        foundation.generated_callables.len(),
        foundation.definition_origins.len(),
        meter,
    )?;
    let mut keys: BTreeMap<_, _> = std::mem::take(&mut foundation.generated_callables)
        .into_iter()
        .map(|r| (r.id(), r))
        .collect();
    let mut origins: BTreeMap<_, _> = std::mem::take(&mut foundation.definition_origins)
        .into_iter()
        .map(|r| (r.subject(), r))
        .collect();
    crate::production::visit_source_callable_reference_keys(
        export,
        meter,
        &mut |record, origin, meter| {
            let bytes = scoop_wire::encoded_length(&record)
                .and_then(|n| scoop_wire::encoded_length(&origin).map(|m| n.saturating_add(m)))
                .map_err(|e| Error::IdentityDerivation {
                    table: HirFoundationTable::GeneratedCallable,
                    reason: e.to_string(),
                })?;
            meter
                .check_semantic_leaf(bytes, &path)
                .map_err(Error::DefaultSourceResource)?;
            meter
                .charge_owned_bytes(bytes, &path)
                .map_err(Error::DefaultSourceResource)?;
            meter
                .charge_work(
                    bytes.saturating_add(
                        u64::from(keys.len().max(1).ilog2())
                            + u64::from(origins.len().max(1).ilog2())
                            + 2,
                    ),
                    &path,
                )
                .map_err(Error::DefaultSourceResource)?;
            match keys.entry(record.id()) {
                Entry::Vacant(entry) => {
                    meter
                        .charge_collection_slots(1, &path)
                        .map_err(Error::DefaultSourceResource)?;
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
                    meter
                        .charge_collection_slots(1, &path)
                        .map_err(Error::DefaultSourceResource)?;
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
        },
    )?;
    charge_tables(keys.len(), origins.len(), meter)?;
    foundation.set_generated_callables(keys.into_values().collect())?;
    foundation.set_definition_origins(origins.into_values().collect())
}

fn charge_tables(
    keys: usize,
    origins: usize,
    meter: &mut BudgetMeter,
) -> Result<(), HirFoundationBuildError> {
    let path = WirePath::root();
    for count in [keys, origins] {
        meter
            .check_table_entries(count as u64, &path)
            .map_err(HirFoundationBuildError::DefaultSourceResource)?;
        // Includes map nodes, result vectors and canonical dependency-order scratch.
        meter
            .charge_collection_slots((count as u64).saturating_mul(5), &path)
            .map_err(HirFoundationBuildError::DefaultSourceResource)?;
        meter
            .charge_work(
                (count as u64).saturating_mul(u64::from(count.max(1).ilog2()) + 4),
                &path,
            )
            .map_err(HirFoundationBuildError::DefaultSourceResource)?;
    }
    Ok(())
}
