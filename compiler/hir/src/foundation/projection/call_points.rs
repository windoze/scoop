//! Required call locations come from the sealed executable HIR occurrences.

use super::*;

pub(super) fn complete(
    output: &crate::DependencyHirOutput,
    foundation: &mut CanonicalHirFoundation,
    meter: &mut BudgetMeter,
) -> Result<(), HirFoundationBuildError> {
    let calls = output
        .committed_dependency_call_occurrences(meter)
        .map_err(|source| HirFoundationBuildError::DependencyCalls(Box::new(source)))?;
    if calls.is_empty() {
        return Ok(());
    }
    let path = scoop_wire::WirePath::root();
    let resources = |source| {
        HirFoundationBuildError::DependencyCalls(Box::new(
            crate::DependencyCallOccurrenceError::Resource(source),
        ))
    };
    let mut locations = Vec::new();
    meter
        .charge_owned_bytes(
            (calls.len() as u64).saturating_mul(
                2 * (std::mem::size_of::<(&SourceIdentity, [u64; 2])>()
                    + std::mem::size_of::<crate::SourcePointRecord>()) as u64,
            ),
            &path,
        )
        .map_err(resources)?;
    meter
        .try_reserve_collection_slots(&mut locations, calls.len().saturating_mul(2), &path)
        .map_err(resources)?;
    let export = output.output().export.module();
    for call in calls {
        let origin = call.origin();
        for (file, span) in [
            (origin.definition.file, origin.definition.span),
            (origin.evaluation.file, origin.evaluation.span),
        ] {
            // The occurrence traversal already checked both file indices and spans.
            locations.push((
                &export.source_files[file as usize].identity,
                [u64::from(span.start), u64::from(span.end)],
            ));
        }
    }
    for source in &export.source_files {
        let count = locations
            .iter()
            .filter(|(identity, _)| **identity == source.identity)
            .count() as u64;
        meter
            .charge_work(
                (source.source.len() as u64)
                    .saturating_mul(count.saturating_mul(2) + 1)
                    .saturating_add(locations.len() as u64),
                &path,
            )
            .map_err(resources)?;
    }
    foundation.set_sources(super::source_points::source_records_with_locations(
        &export.source_files,
        &foundation.definition_origins,
        &[],
        &foundation.sources,
        locations,
    )?)
}
