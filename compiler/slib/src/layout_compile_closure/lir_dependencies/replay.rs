use std::collections::BTreeMap;

use scoop_identity::RepresentationRole;
use scoop_wire::BudgetMeter;

use super::{SharedLirDependencyGraphError as Error, *};

/// Replays the graph from shared HIR occurrences and complete export tables.
/// This neither consumes candidate MIR roots nor infers uses from link imports.
pub fn replay_shared_lir_dependency_graph(
    source: scoop_hir::SharedTypeMetadataV1<'_>,
    layout: &lir::DependencyResolvedCrossConeLayoutAbiSectionV1,
    dependencies: &[&lir::LayoutAbiExportConstituentsV1],
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let exports = layout.exports();
    if source.provider != exports.provider() {
        return Err(Error::InputProvider {
            source: source.provider,
            layout: exports.provider(),
        });
    }
    let path = WirePath::root();
    meter.check_table_entries(dependencies.len() as u64, &path)?;
    let mut by_provider = BTreeMap::new();
    for dependency in dependencies {
        let provider = dependency.provider();
        meter.charge_work(1 + u64::from(dependencies.len().max(1).ilog2()), &path)?;
        if provider == source.provider || by_provider.contains_key(&provider) {
            return Err(Error::DependencyProvider(provider));
        }
        if dependency.target_profile() != exports.target_profile() {
            return Err(Error::DependencyTarget(provider));
        }
        meter.charge_collection_slots(1, &path)?;
        meter.charge_owned_bytes(
            std::mem::size_of::<(ConeIdentity, &lir::LayoutAbiExportConstituentsV1)>() as u64,
            &path,
        )?;
        by_provider.insert(provider, *dependency);
    }
    let mut providers = Vec::new();
    meter.try_reserve_collection_slots(&mut providers, by_provider.len(), &path)?;
    providers.extend(by_provider.keys().copied());
    let references = source.public.external_references();
    references
        .validate_type_site_relations(source.provider, source.identities, &providers, meter)
        .map_err(|error| Error::TypeOccurrences(Box::new(error)))?;
    let roots = references
        .materialized_type_dependencies(source.provider, source.identities, meter)
        .map_err(|error| Error::TypeOccurrences(Box::new(error)))?;
    let mut committed = Vec::new();
    meter.charge_owned_bytes(
        (roots.len() as u64)
            .saturating_mul(std::mem::size_of::<lir::LayoutAbiDependencyV1>() as u64),
        &path,
    )?;
    meter.try_reserve_collection_slots(&mut committed, roots.len(), &path)?;
    for (provider, exact) in roots {
        meter.charge_work(1 + u64::from(by_provider.len().max(1).ilog2()), &path)?;
        let dependency = by_provider
            .get(&provider)
            .ok_or(Error::DependencyProvider(provider))?;
        meter.charge_work(dependency.layouts().records().len() as u64, &path)?;
        let value = dependency
            .layouts()
            .find_exact_role(exact, RepresentationRole::ManagedValue)
            .ok_or(Error::MissingTypeLayout { provider, exact })?;
        committed.push(lir::LayoutAbiDependencyV1::new(
            provider,
            lir::LayoutAbiSemanticTargetV1::Layout(value.identity().layout()),
        ));
    }
    meter.charge_work(
        (committed.len() as u64).saturating_mul(1 + u64::from(committed.len().max(1).ilog2())),
        &path,
    )?;
    committed.sort_unstable();
    committed.dedup();
    layout.replay_dependency_closure::<Infallible>(dependencies, &committed, meter)?;
    Ok(())
}
