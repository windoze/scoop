use scoop_wire::BudgetMeter;

use super::{SharedMirDependencyGraphError as Error, *};

/// Replays the complete graph from shared HIR occurrences. Candidate selected
/// records, MIR type lookup tables and physical imports cannot add source roots.
pub fn replay_shared_mir_dependency_graph(
    source: scoop_hir::SharedTypeMetadataV1<'_>,
    mir: &mir::DependencyResolvedCrossConeMirTypeBridgeSectionV1,
    units: &[mir::MirTypeBridgeInitializationUnitV1],
    dependencies: &[mir::MirTypeBridgeDependencyViewV1<'_>],
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    if source.provider != mir.provider() {
        return Err(Error::InputProvider {
            source: source.provider,
            mir: mir.provider(),
        });
    }
    let path = WirePath::root();
    let mut providers = Vec::new();
    meter.try_reserve_collection_slots(&mut providers, dependencies.len(), &path)?;
    providers.extend(dependencies.iter().map(|view| view.provider()));
    let references = source.public.external_references();
    references
        .validate_type_site_relations(source.provider, source.identities, &providers, meter)
        .map_err(|error| Error::TypeOccurrences(Box::new(error)))?;
    let types = references
        .materialized_type_dependencies(source.provider, source.identities, meter)
        .map_err(|error| Error::TypeOccurrences(Box::new(error)))?;
    let shapes = references
        .materialized_shape_dependencies(source.provider, source.identities, meter)
        .map_err(|error| Error::TypeOccurrences(Box::new(error)))?;
    let count = types.len().saturating_add(shapes.len());
    let mut committed = Vec::new();
    meter.charge_owned_bytes(
        (count as u64).saturating_mul(std::mem::size_of::<mir::MirTypeBridgeDependencyV1>() as u64),
        &path,
    )?;
    meter.try_reserve_collection_slots(&mut committed, count, &path)?;
    committed.extend(types.into_iter().map(|(provider, exact)| {
        mir::MirTypeBridgeDependencyV1::new(provider, mir::MirTypeBridgeTargetV1::Type(exact))
    }));
    committed.extend(shapes.into_iter().map(|(provider, owner)| {
        mir::MirTypeBridgeDependencyV1::new(
            provider,
            mir::MirTypeBridgeTargetV1::ShapeSupport(owner),
        )
    }));
    meter.charge_work(
        (committed.len() as u64).saturating_mul(1 + u64::from(committed.len().max(1).ilog2())),
        &path,
    )?;
    committed.sort_unstable();
    committed.dedup();
    mir.replay_dependency_closure::<Infallible>(
        units,
        dependencies,
        &committed,
        source.identities,
        meter,
    )?;
    Ok(())
}
