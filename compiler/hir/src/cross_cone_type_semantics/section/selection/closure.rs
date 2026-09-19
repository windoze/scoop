use super::*;
use std::collections::BTreeMap;
mod provenance;
mod queue;
use queue::{Queued, enqueue};

type Exports<'a> = exports::CheckedTypeSectionExportsV1<'a>;

pub(in crate::cross_cone_type_semantics::section) fn validate<'a, F, A, E>(
    local: &Exports<'a>,
    dependencies: &[&'a CheckedCrossConeTypeSemanticsSectionV1<'a>],
    foundation: &F,
    authority: &A,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<Vec<CheckedSelectedTypeUseV1<'a>>, TypeSelectionValidationError<E>>
where
    F: TypeSectionFoundationSemanticAuthority<E>,
    A: CommittedTypeUseSemanticAuthorityV1<E>,
{
    use TypeSelectionValidationError as Error;
    let roots = authority.committed_roots().map_err(Error::Source)?;
    meter.check_table_entries(roots.len() as u64, path)?;
    meter.charge_work(roots.len() as u64, path)?;
    let mut pending = Vec::new();
    let mut visited = BTreeMap::new();
    let mut external = BTreeMap::new();
    let context = TypeSectionUseContextV1 { exports: local };
    for (index, root) in roots.iter().enumerate() {
        let at = path.clone().field(1).index(index as u64);
        meter.charge_nodes(1, &at)?;
        let request = authority.root_request(root).map_err(Error::Source)?;
        let origin = authority.root_origin(root).map_err(Error::Source)?;
        provenance::validate(local, dependencies, origin, meter, &at)?;
        let target = resolve(local, dependencies, request, foundation, meter, &at)?;
        // All actual uses replay, even when another use already queued this target.
        authority
            .validate_root(root, target, context, meter, &at)
            .map_err(Error::Source)?;
        enqueue(
            Queued { request, depth: 1 },
            &mut pending,
            &mut visited,
            &mut external,
            local.provider,
            meter,
            &at,
        )?;
    }
    while let Some(Queued { request, depth }) = pending.pop() {
        let parent = resolve(local, dependencies, request, foundation, meter, path)?;
        let edges = authority.semantic_edges(parent).map_err(Error::Source)?;
        meter.check_table_entries(edges.len() as u64, path)?;
        meter.charge_work(edges.len() as u64, path)?;
        for (index, edge) in edges.iter().enumerate() {
            let at = path.clone().field(2).index(index as u64);
            meter.charge_edges(1, &at)?;
            let request = authority.edge_request(edge).map_err(Error::Source)?;
            let origin = authority.edge_origin(edge).map_err(Error::Source)?;
            provenance::validate(local, dependencies, origin, meter, &at)?;
            let target = resolve(local, dependencies, request, foundation, meter, &at)?;
            authority
                .validate_edge(parent, edge, target, context, meter, &at)
                .map_err(Error::Source)?;
            enqueue(
                Queued {
                    request,
                    depth: depth.checked_add(1).ok_or_else(|| {
                        scoop_wire::WireError::new(
                            scoop_wire::WireErrorKind::IntegerOutOfRange,
                            at.clone(),
                            None,
                        )
                    })?,
                },
                &mut pending,
                &mut visited,
                &mut external,
                local.provider,
                meter,
                &at,
            )?;
        }
    }
    let supplied = local.candidate.selected.records();
    meter.check_table_entries(supplied.len() as u64, path)?;
    meter.charge_work(
        (supplied.len() as u64 + external.len() as u64).saturating_mul(128),
        path,
    )?;
    if !supplied.iter().copied().eq(external.values().copied()) {
        return Err(Error::Inventory);
    }
    let mut selected = Vec::new();
    meter.try_reserve_collection_slots(&mut selected, external.len(), path)?;
    for request in external.into_values() {
        let terminal = terminal(dependencies, request.provider(), meter, path)?;
        let target = targets::validate(request, local, &terminal.exports, foundation, meter, path)?;
        selected.push(CheckedSelectedTypeUseV1 { target, terminal });
    }
    Ok(selected)
}

fn terminal<'a, E>(
    dependencies: &[&'a CheckedCrossConeTypeSemanticsSectionV1<'a>],
    provider: ConeIdentity,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<&'a CheckedCrossConeTypeSemanticsSectionV1<'a>, TypeSelectionValidationError<E>> {
    meter.charge_work((dependencies.len() as u64 + 1).ilog2() as u64 + 1, path)?;
    dependencies
        .binary_search_by_key(&provider, |section| section.provider())
        .ok()
        .map(|index| dependencies[index])
        .ok_or(TypeSelectionValidationError::MissingProvider(provider))
}

fn resolve<'a, F: TypeSectionFoundationSemanticAuthority<E>, E>(
    local: &Exports<'a>,
    dependencies: &[&'a CheckedCrossConeTypeSemanticsSectionV1<'a>],
    request: SelectedExternalTypeUseV1,
    foundation: &F,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<CheckedTypeSelectionTargetV1<'a>, TypeSelectionValidationError<E>> {
    let provider = if request.provider() == local.provider {
        local
    } else {
        &terminal(dependencies, request.provider(), meter, path)?.exports
    };
    targets::validate(request, local, provider, foundation, meter, path)
}
