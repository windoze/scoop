use std::collections::BTreeMap;

use scoop_identity::{CborIdentityKey, CborIdentityRecord, PersistentId, ValidatedIdentityGraph};

use super::TypeFoundationBindingError;

pub(super) fn index<I, K>(
    records: &[CborIdentityRecord<I, K>],
) -> Result<BTreeMap<I, &K>, TypeFoundationBindingError>
where
    I: PersistentId,
    K: CborIdentityKey<I>,
{
    Ok(records
        .iter()
        .map(|record| (record.id(), record.key()))
        .collect())
}

pub(super) fn select<'a, I, K>(
    required: &[I],
    available: &BTreeMap<I, &'a K>,
    identities: &ValidatedIdentityGraph,
    missing: impl Fn(I) -> TypeFoundationBindingError,
) -> Result<BTreeMap<I, &'a K>, TypeFoundationBindingError>
where
    I: PersistentId + 'static,
    K: CborIdentityKey<I> + Eq + Clone + Send + Sync + 'static,
{
    let mut result = BTreeMap::new();
    for id in required {
        let key = available.get(id).copied().ok_or_else(|| missing(*id))?;
        verify(*id, key, identities)?;
        result.insert(*id, key);
    }
    Ok(result)
}

pub(super) fn verify<I, K>(
    id: I,
    key: &K,
    identities: &ValidatedIdentityGraph,
) -> Result<(), TypeFoundationBindingError>
where
    I: PersistentId + 'static,
    K: CborIdentityKey<I> + Eq + Clone + Send + Sync + 'static,
{
    // Canonical foundation keys have already passed identity decoding. This
    // comparison only borrows them; no unchecked keys or deep copies escape.

    let checked = identities
        .canonical_key::<I, K>(id)
        .map_err(|error| TypeFoundationBindingError::Identity(error.to_string()))?;
    if checked.as_ref() != key {
        return Err(TypeFoundationBindingError::CanonicalKeyMismatch);
    }
    Ok(())
}
