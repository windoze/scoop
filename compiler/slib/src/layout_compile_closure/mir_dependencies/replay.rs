use super::{SharedMirDependencyGraphError as Error, *};

/// Checks shared type dependencies and the complete MIR reference graph.
/// Implicit calls retain ordinary MIR references without a second HIR call record.
pub fn replay_shared_mir_dependency_graph(
    source: scoop_hir::SharedTypeMetadataV1<'_>,
    source_dependencies: &[scoop_hir::SharedTypeMetadataV1<'_>],
    mir: &mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
    units: &[mir::MirTypeBridgeInitializationUnitV1],
    dependencies: &[mir::MirTypeBridgeDependencyViewV1<'_>],
) -> Result<(), Error> {
    if source.provider != mir.provider() {
        return Err(Error::InputProvider {
            source: source.provider,
            mir: mir.provider(),
        });
    }
    let path = WirePath::root();
    super::initialization::replay(source, source_dependencies, mir, units)?;
    let references = source.public.external_references();
    let types = references
        .materialized_type_dependencies(source.provider, source.identities)
        .map_err(|error| Error::TypeOccurrences(Box::new(error)))?;
    let shapes = references
        .materialized_shape_dependencies(source.provider, source.identities)
        .map_err(|error| Error::TypeOccurrences(Box::new(error)))?;
    let count = types.len().saturating_add(shapes.len());
    let mut committed = Vec::new();

    scoop_wire::allocation::try_reserve(&mut committed, count, &path)?;
    committed.extend(types.into_iter().map(|(provider, exact)| {
        mir::MirTypeBridgeDependencyV1::new(provider, mir::MirTypeBridgeTargetV1::Type(exact))
    }));
    committed.extend(shapes.into_iter().map(|(provider, owner)| {
        mir::MirTypeBridgeDependencyV1::new(
            provider,
            mir::MirTypeBridgeTargetV1::ShapeSupport(owner),
        )
    }));

    for reference in references.records() {
        if !reference
            .roles()
            .contains(scoop_hir::ExternalHirReferenceRoleV1::ConcreteSelectedUse)
        {
            continue;
        }
        let target = crate::hir_dependency_calls::concrete_callable(reference.target())
            .map_err(|source| Error::CallSites(Box::new(source)))?;
        let Some(target) = target else {
            continue;
        };
        if dependencies.iter().any(|view| {
            view.provider() == reference.origin()
                && view.exports().callables().get(target).is_some()
        }) {
            committed.push(mir::MirTypeBridgeDependencyV1::new(
                reference.origin(),
                mir::MirTypeBridgeTargetV1::Callable(target),
            ));
        }
    }

    committed.extend(
        mir.selected_relations().iter().copied().filter(|relation| {
            matches!(relation.target(), mir::MirTypeBridgeTargetV1::Callable(_))
        }),
    );

    committed.sort_unstable();
    committed.dedup();
    mir.replay_dependency_closure(units, dependencies, &committed, source.identities)?;
    Ok(())
}
