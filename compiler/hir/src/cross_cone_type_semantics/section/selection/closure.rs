use super::*;
use std::collections::BTreeMap;
mod provenance;
mod queue;
use queue::enqueue;

type Exports<'a> = exports::CheckedTypeSectionExportsV1<'a>;

pub(in crate::cross_cone_type_semantics::section) fn validate<'a, F, A, E>(
    local: &Exports<'a>,
    dependencies: &[&'a CheckedCrossConeTypeSemanticsSectionV1<'a>],
    foundation: &F,
    authority: &A,

    path: &WirePath,
) -> Result<Vec<CheckedSelectedTypeUseV1<'a>>, TypeSelectionValidationError<E>>
where
    F: TypeSectionFoundationSemanticAuthority<E>,
    A: CommittedTypeUseSemanticAuthorityV1<E>,
{
    use TypeSelectionValidationError as Error;
    let roots = authority.committed_roots().map_err(Error::Source)?;

    let mut pending = Vec::new();
    let mut visited = BTreeMap::new();
    let mut external = BTreeMap::new();
    let context = TypeSectionUseContextV1 { exports: local };
    for (index, root) in roots.iter().enumerate() {
        let at = path.clone().field(1).index(index as u64);

        let request = authority.root_request(root).map_err(Error::Source)?;
        let origin = authority.root_origin(root).map_err(Error::Source)?;
        provenance::validate(local, dependencies, origin)?;
        let target = resolve(local, dependencies, request, foundation, &at)?;
        // All actual uses replay, even when another use already request this target.
        authority
            .validate_root(root, target, context, &at)
            .map_err(Error::Source)?;
        enqueue(
            request,
            &mut pending,
            &mut visited,
            &mut external,
            local.provider,
            &at,
        )?;
    }
    while let Some(request) = pending.pop() {
        let parent = resolve(local, dependencies, request, foundation, path)?;
        let edges = authority.semantic_edges(parent).map_err(Error::Source)?;

        for (index, edge) in edges.iter().enumerate() {
            let at = path.clone().field(2).index(index as u64);

            let request = authority.edge_request(edge).map_err(Error::Source)?;
            let origin = authority.edge_origin(edge).map_err(Error::Source)?;
            provenance::validate(local, dependencies, origin)?;
            let target = resolve(local, dependencies, request, foundation, &at)?;
            authority
                .validate_edge(parent, edge, target, context, &at)
                .map_err(Error::Source)?;
            enqueue(
                request,
                &mut pending,
                &mut visited,
                &mut external,
                local.provider,
                &at,
            )?;
        }
    }
    let supplied = local.candidate.selected.records();

    if !supplied.iter().copied().eq(external.values().copied()) {
        return Err(Error::Inventory);
    }
    let mut selected = Vec::new();
    scoop_wire::allocation::try_reserve(&mut selected, external.len(), path)?;
    for request in external.into_values() {
        let terminal = terminal(dependencies, request.provider())?;
        let target = targets::validate(request, local, &terminal.exports, foundation, path)?;
        selected.push(CheckedSelectedTypeUseV1 { target, terminal });
    }
    Ok(selected)
}

fn terminal<'a, E>(
    dependencies: &[&'a CheckedCrossConeTypeSemanticsSectionV1<'a>],
    provider: ConeIdentity,
) -> Result<&'a CheckedCrossConeTypeSemanticsSectionV1<'a>, TypeSelectionValidationError<E>> {
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

    path: &WirePath,
) -> Result<CheckedTypeSelectionTargetV1<'a>, TypeSelectionValidationError<E>> {
    let provider = if request.provider() == local.provider {
        local
    } else {
        &terminal(dependencies, request.provider())?.exports
    };
    targets::validate(request, local, provider, foundation, path)
}
