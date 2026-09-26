use super::{SharedMirDependencyGraphError as Error, *};

/// Replays the complete graph from shared HIR occurrences. Candidate selected
/// records, MIR type lookup tables and physical imports cannot add source roots.
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
    let mut providers = Vec::new();
    scoop_wire::allocation::try_reserve(&mut providers, dependencies.len(), &path)?;
    providers.extend(dependencies.iter().map(|view| view.provider()));
    let references = source.public.external_references();
    references
        .validate_type_site_relations(source.provider, source.identities, &providers)
        .map_err(|error| Error::TypeOccurrences(Box::new(error)))?;
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

    committed.sort_unstable();
    committed.dedup();
    mir.replay_dependency_closure(units, dependencies, &committed, source.identities)?;
    Ok(())
}
