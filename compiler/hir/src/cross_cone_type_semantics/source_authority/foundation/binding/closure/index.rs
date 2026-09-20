use super::*;
use scoop_wire::{WireEncode, WireErrorKind, encoded_length};

pub(super) fn compose<'a>(
    root: TypeFoundationSourceProviderV1<'a>,
    dependencies: &[TypeFoundationSourceProviderV1<'a>],
    meter: &mut BudgetMeter,
) -> Result<TypeFoundationSourceClosureV1<'a>, TypeFoundationReplayError> {
    let path = WirePath::root();
    meter.check_semantic_depth(1, &path)?;
    meter.charge_nodes(1, &path)?;
    meter.check_table_entries(dependencies.len() as u64, &path)?;
    meter.charge_work(dependencies.len() as u64, &path)?;
    let mut previous = None;
    for dependency in dependencies {
        let provider = dependency.provider();
        if provider == root.provider() || previous.is_some_and(|value| value >= provider) {
            return Err(TypeFoundationReplayError::DependencyOrder);
        }
        previous = Some(provider);
    }
    let mut result = TypeFoundationSourceClosureV1 {
        root,
        providers: BTreeMap::new(),
        exacts: BTreeMap::new(),
        nominals: BTreeMap::new(),
        generated: BTreeMap::new(),
        accessors: BTreeMap::new(),
        facts: BTreeMap::new(),
    };
    for provider in std::iter::once(root).chain(dependencies.iter().copied()) {
        charge_insert(result.providers.len(), meter)?;
        result.providers.insert(provider.provider(), provider);
        extend_keys(&mut result.exacts, &provider.source.exact_keys, meter)?;
        extend_keys(
            &mut result.generated,
            &provider.source.generated_keys,
            meter,
        )?;
        extend_keys(&mut result.accessors, &provider.source.accessor_keys, meter)?;
        for owner in provider.source.nominal_keys.keys() {
            charge_insert(result.nominals.len(), meter)?;
            if result.nominals.insert(*owner, provider).is_some() {
                return Err(TypeFoundationReplayError::DuplicateNominal(*owner));
            }
        }
        for record in provider.entries().fact_shapes.records() {
            charge_insert(result.facts.len(), meter)?;
            meter.charge_work(
                u64::from(provider.source.exact_keys.len().max(1).ilog2()) + 1,
                &path,
            )?;
            provider.source.exact_type_key(record.exact())?;
            if result
                .facts
                .insert(record.exact(), (provider.provider(), record.shape()))
                .is_some()
            {
                return Err(TypeFoundationReplayError::DuplicateFact(record.exact()));
            }
        }
    }
    validate_dependencies(&result, meter)?;
    Ok(result)
}

fn validate_dependencies(
    sources: &TypeFoundationSourceClosureV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), TypeFoundationReplayError> {
    let path = WirePath::root();
    let provider_work = u64::from(sources.providers.len().max(1).ilog2()) + 1;
    let fact_work = u64::from(sources.facts.len().max(1).ilog2()) + 1;
    for source in sources.providers.values() {
        let dependencies = source.entries().dependency_facts.records();
        meter.check_table_entries(dependencies.len() as u64, &path)?;
        for fact in dependencies {
            meter.charge_work(provider_work + fact_work, &path)?;
            if !sources.providers.contains_key(&fact.provider) {
                return Err(TypeFoundationReplayError::MissingProvider(fact.provider));
            }
            if fact.provider == source.provider()
                || sources
                    .facts
                    .get(&fact.exact)
                    .map(|(provider, _)| *provider)
                    != Some(fact.provider)
            {
                return Err(TypeFoundationReplayError::DependencyFact {
                    provider: fact.provider,
                    exact: fact.exact,
                });
            }
        }
    }
    Ok(())
}

fn extend_keys<'a, I: Copy + Ord, K: Eq + WireEncode>(
    output: &mut BTreeMap<I, &'a K>,
    incoming: &BTreeMap<I, &'a K>,
    meter: &mut BudgetMeter,
) -> Result<(), TypeFoundationReplayError> {
    let path = WirePath::root();
    for (id, key) in incoming {
        charge_insert(output.len(), meter)?;
        if let Some(previous) = output.insert(*id, key) {
            for compared in [previous, *key] {
                let bytes = encoded_length(compared).map_err(|_| {
                    WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None)
                })?;
                meter.check_semantic_leaf(bytes, &path)?;
                meter.charge_work(bytes, &path)?;
            }
            if previous != *key {
                return Err(TypeFoundationReplayError::SharedKeyMismatch);
            }
        }
    }
    Ok(())
}

fn charge_insert(count: usize, meter: &mut BudgetMeter) -> Result<(), WireError> {
    let path = WirePath::root();
    meter.check_table_entries((count as u64).saturating_add(1), &path)?;
    meter.charge_nodes(1, &path)?;
    meter.charge_collection_slots(1, &path)?;
    meter.charge_work(u64::from(count.max(1).ilog2()) + 1, &path)
}
