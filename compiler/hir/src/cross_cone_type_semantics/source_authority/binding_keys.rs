use std::collections::BTreeMap;

use scoop_identity::{CborIdentityKey, CborIdentityRecord, PersistentId, ValidatedIdentityGraph};
use scoop_wire::{BudgetMeter, WireError, WirePath, encoded_length};

use super::TypeFoundationBindingError;

pub(super) fn charge_map(
    count: usize,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), WireError> {
    meter.check_table_entries(count as u64, path)?;
    meter.charge_nodes(count as u64, path)?;
    meter.charge_collection_slots(count as u64, path)?;
    meter.charge_work(
        (count as u64).saturating_mul(u64::from(count.max(1).ilog2()) + 1),
        path,
    )
}

pub(super) fn index<'a, I, K>(
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

pub(super) fn select<'a, I, K>(
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

pub(super) fn verify<I, K>(
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
