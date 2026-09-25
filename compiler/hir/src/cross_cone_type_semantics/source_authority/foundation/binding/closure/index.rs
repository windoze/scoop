use super::*;
use scoop_wire::WireEncode;

pub(super) fn compose<'a>(
    root: TypeFoundationSourceProviderV1<'a>,
    dependencies: &[TypeFoundationSourceProviderV1<'a>],
) -> Result<TypeFoundationSourceClosureV1<'a>, TypeFoundationReplayError> {
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
        result.providers.insert(provider.provider(), provider);
        extend_keys(&mut result.exacts, &provider.source.exact_keys)?;
        extend_keys(&mut result.generated, &provider.source.generated_keys)?;
        extend_keys(&mut result.accessors, &provider.source.accessor_keys)?;
        for owner in provider.source.nominal_keys.keys() {
            if result.nominals.insert(*owner, provider).is_some() {
                return Err(TypeFoundationReplayError::DuplicateNominal(*owner));
            }
        }
        for record in provider.entries().fact_shapes.records() {
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
    validate_dependencies(&result)?;
    Ok(result)
}

fn validate_dependencies(
    sources: &TypeFoundationSourceClosureV1<'_>,
) -> Result<(), TypeFoundationReplayError> {
    for source in sources.providers.values() {
        let dependencies = source.entries().dependency_facts.records();

        for fact in dependencies {
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
) -> Result<(), TypeFoundationReplayError> {
    for (id, key) in incoming {
        if let Some(previous) = output.insert(*id, key) {
            if previous != *key {
                return Err(TypeFoundationReplayError::SharedKeyMismatch);
            }
        }
    }
    Ok(())
}
