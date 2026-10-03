//! Required call locations come from the sealed executable HIR occurrences.

use super::*;

pub(super) fn complete(
    output: &crate::DependencyHirOutput,
    foundation: &mut CanonicalHirFoundation,
) -> Result<(), HirFoundationBuildError> {
    let calls = output
        .committed_dependency_call_occurrences()
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

    scoop_wire::allocation::try_reserve(&mut locations, calls.len().saturating_mul(2), &path)
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

    foundation.set_sources(super::source_points::source_records_with_locations(
        &export.source_files,
        &foundation.definition_origins,
        &[],
        &foundation.sources,
        locations,
    )?)
}
