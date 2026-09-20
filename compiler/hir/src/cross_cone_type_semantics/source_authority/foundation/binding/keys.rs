use super::*;
use scoop_identity::{CborIdentityKey, CborIdentityRecord, PersistentId};
use scoop_wire::encoded_length;

pub(super) fn bind<'a>(
    source: &'a TypeFoundationSourceAuthorityV1,
    foundation: &'a OdrFreeHirFoundation,
    identities: &ValidatedIdentityGraph,
    meter: &mut BudgetMeter,
) -> Result<BoundTypeFoundationSourcesV1<'a>, TypeFoundationBindingError> {
    let root = WirePath::root();
    meter.check_semantic_depth(1, &root)?;
    meter.charge_nodes(1, &root)?;
    let canonical = foundation.as_canonical();
    let entries = source.entries();
    let exacts = index(
        canonical.type_source_exact_records(),
        meter,
        &root.clone().field(2),
    )?;
    let exact_keys = select(
        entries.exact_keys.values(),
        &exacts,
        identities,
        meter,
        &root.clone().field(2),
        TypeFoundationBindingError::MissingExact,
    )?;

    let concrete = index(
        canonical.type_source_nominal_records(),
        meter,
        &root.clone().field(3),
    )?;
    let generic = index(
        canonical.type_source_generic_records(),
        meter,
        &root.clone().field(3),
    )?;
    let mut nominal_keys = BTreeMap::new();
    charge_map(
        entries.sources.records().len(),
        meter,
        &root.clone().field(3),
    )?;
    for record in entries.sources.records() {
        let owner = record.owner();
        let key = match owner {
            SourceNominalId::Concrete(id) => {
                let key = concrete
                    .get(&id)
                    .copied()
                    .ok_or(TypeFoundationBindingError::MissingNominal(owner))?;
                verify(id, key, identities, meter, &root.clone().field(3))?;
                key
            }
            SourceNominalId::GenericTemplate(id) => {
                let key = generic
                    .get(&id)
                    .copied()
                    .ok_or(TypeFoundationBindingError::MissingNominal(owner))?;
                verify(id, key, identities, meter, &root.clone().field(3))?;
                key
            }
        };
        if key.origin() != entries.provider {
            return Err(TypeFoundationBindingError::ForeignNominal(owner));
        }
        nominal_keys.insert(owner, key);
    }
    let generated = index(
        canonical.type_source_generated_records(),
        meter,
        &root.clone().field(5),
    )?;
    let generated_keys = select(
        entries.generated_nominals.values(),
        &generated,
        identities,
        meter,
        &root.clone().field(5),
        TypeFoundationBindingError::MissingGenerated,
    )?;
    let accessors = index(
        canonical.type_source_accessor_records(),
        meter,
        &root.clone().field(6),
    )?;
    let accessor_keys = select(
        entries.accessor_keys.values(),
        &accessors,
        identities,
        meter,
        &root.clone().field(6),
        TypeFoundationBindingError::MissingAccessor,
    )?;
    Ok(BoundTypeFoundationSourcesV1 {
        source,
        foundation,
        exact_keys,
        nominal_keys,
        generated_keys,
        accessor_keys,
    })
}

fn charge_map(count: usize, meter: &mut BudgetMeter, path: &WirePath) -> Result<(), WireError> {
    meter.check_table_entries(count as u64, path)?;
    meter.charge_nodes(count as u64, path)?;
    meter.charge_collection_slots(count as u64, path)?;
    meter.charge_work(
        (count as u64).saturating_mul(u64::from(count.max(1).ilog2()) + 1),
        path,
    )
}

fn index<'a, I, K>(
    records: &'a [CborIdentityRecord<I, K>],
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<BTreeMap<I, &'a K>, TypeFoundationBindingError>
where
    I: PersistentId,
    K: CborIdentityKey<I>,
{
    charge_map(records.len(), meter, path)?;
    Ok(records
        .iter()
        .map(|record| (record.id(), record.key()))
        .collect())
}

fn select<'a, I, K>(
    required: &[I],
    available: &BTreeMap<I, &'a K>,
    identities: &ValidatedIdentityGraph,
    meter: &mut BudgetMeter,
    path: &WirePath,
    missing: impl Fn(I) -> TypeFoundationBindingError,
) -> Result<BTreeMap<I, &'a K>, TypeFoundationBindingError>
where
    I: PersistentId + 'static,
    K: CborIdentityKey<I> + Eq + Clone + Send + Sync + 'static,
{
    charge_map(required.len(), meter, path)?;
    let mut result = BTreeMap::new();
    for id in required {
        meter.charge_work(u64::from(available.len().max(1).ilog2()) + 1, path)?;
        let key = available.get(id).copied().ok_or_else(|| missing(*id))?;
        verify(*id, key, identities, meter, path)?;
        result.insert(*id, key);
    }
    Ok(result)
}

fn verify<I, K>(
    id: I,
    key: &K,
    identities: &ValidatedIdentityGraph,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeFoundationBindingError>
where
    I: PersistentId + 'static,
    K: CborIdentityKey<I> + Eq + Clone + Send + Sync + 'static,
{
    // Canonical foundation keys have already passed identity decoding. This
    // comparison only borrows them; no unchecked keys or deep copies escape.
    let bytes = encoded_length(key)
        .map_err(|error| TypeFoundationBindingError::Identity(error.to_string()))?;
    meter.check_semantic_leaf(bytes, path)?;
    meter.charge_work(bytes, path)?;
    let checked = identities
        .canonical_key::<I, K>(id)
        .map_err(|error| TypeFoundationBindingError::Identity(error.to_string()))?;
    if checked.as_ref() != key {
        return Err(TypeFoundationBindingError::CanonicalKeyMismatch);
    }
    Ok(())
}
