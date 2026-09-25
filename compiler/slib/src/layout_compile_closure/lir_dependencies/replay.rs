use std::collections::BTreeMap;

use scoop_identity::RepresentationRole;

use super::{SharedLirDependencyGraphError as Error, *};

/// Replays the graph from shared HIR occurrences and complete export tables.
/// This neither consumes candidate MIR roots nor infers uses from link imports.
pub fn replay_shared_lir_dependency_graph(
    source: scoop_hir::SharedTypeMetadataV1<'_>,
    layout: &lir::DependencyResolvedCrossConeLayoutAbiSectionV1,
    dependencies: &[&lir::LayoutAbiExportConstituentsV1],
) -> Result<(), Error> {
    let exports = layout.exports();
    if source.provider != exports.provider() {
        return Err(Error::InputProvider {
            source: source.provider,
            layout: exports.provider(),
        });
    }
    let path = WirePath::root();

    let mut by_provider = BTreeMap::new();
    for dependency in dependencies {
        let provider = dependency.provider();

        if provider == source.provider || by_provider.contains_key(&provider) {
            return Err(Error::DependencyProvider(provider));
        }
        if dependency.target_profile() != exports.target_profile() {
            return Err(Error::DependencyTarget(provider));
        }

        by_provider.insert(provider, *dependency);
    }
    let mut providers = Vec::new();
    scoop_wire::allocation::try_reserve(&mut providers, by_provider.len(), &path)?;
    providers.extend(by_provider.keys().copied());
    let references = source.public.external_references();
    references
        .validate_type_site_relations(source.provider, source.identities, &providers)
        .map_err(|error| Error::TypeOccurrences(Box::new(error)))?;
    let roots = references
        .materialized_type_dependencies(source.provider, source.identities)
        .map_err(|error| Error::TypeOccurrences(Box::new(error)))?;
    let shapes = references
        .materialized_shape_dependencies(source.provider, source.identities)
        .map_err(|error| Error::TypeOccurrences(Box::new(error)))?;
    let count = roots.len().saturating_add(shapes.len());
    let mut committed = Vec::new();

    scoop_wire::allocation::try_reserve(&mut committed, count, &path)?;
    for (provider, exact) in roots {
        let dependency = by_provider
            .get(&provider)
            .ok_or(Error::DependencyProvider(provider))?;

        let value = dependency
            .layouts()
            .find_exact_role(exact, RepresentationRole::ManagedValue)
            .ok_or(Error::MissingTypeLayout { provider, exact })?;
        committed.push(lir::LayoutAbiDependencyV1::new(
            provider,
            lir::LayoutAbiSemanticTargetV1::Layout(value.identity().layout()),
        ));
    }
    committed.extend(shapes.into_iter().map(|(provider, owner)| {
        lir::LayoutAbiDependencyV1::new(
            provider,
            lir::LayoutAbiSemanticTargetV1::ShapeSupport(owner),
        )
    }));

    committed.sort_unstable();
    committed.dedup();
    layout.replay_dependency_closure::<Infallible>(dependencies, &committed)?;
    Ok(())
}
