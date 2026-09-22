//! Canonical record queries over local layers and their resolved dependencies.

use super::*;

#[derive(Clone, Copy)]
enum RecordScope {
    Layer(IdentityLayer),
    Closure,
}

impl RecordScope {
    fn includes(self, layer: Option<IdentityLayer>) -> bool {
        match self {
            Self::Layer(expected) => layer == Some(expected),
            Self::Closure => true,
        }
    }
}

impl ValidatedIdentityGraph {
    /// Reconstructs this artifact's records from one layer, sorted by typed id.
    pub fn records<I, K>(
        &self,
        layer: IdentityLayer,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Vec<CborIdentityRecord<I, K>>, IdentityValidationError>
    where
        I: PersistentId + 'static,
        K: CborIdentityKey<I> + Send + Sync + 'static,
    {
        self.records_in_scope(RecordScope::Layer(layer), meter, path)
    }

    /// Borrows canonical keys from this artifact and its resolved dependency
    /// closure. Imported records keep their ownership and are not redeclared.
    pub fn closure_records<I, K>(
        &self,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Vec<CborIdentityRecord<I, K>>, IdentityValidationError>
    where
        I: PersistentId + 'static,
        K: CborIdentityKey<I> + Send + Sync + 'static,
    {
        self.records_in_scope(RecordScope::Closure, meter, path)
    }

    fn records_in_scope<I, K>(
        &self,
        scope: RecordScope,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Vec<CborIdentityRecord<I, K>>, IdentityValidationError>
    where
        I: PersistentId + 'static,
        K: CborIdentityKey<I> + Send + Sync + 'static,
    {
        if matches!(scope, RecordScope::Closure) {
            meter
                .charge_work((self.candidates.len() as u64).saturating_mul(2), path)
                .map_err(IdentityValidationError::Resource)?;
        }
        let record_count = self
            .candidates
            .iter()
            .filter(|(node, candidate)| {
                node.kind == I::KIND
                    && scope.includes(candidate.layer)
                    && self
                        .canonical_keys
                        .contains_key(&CanonicalKeySlot::new::<I, K>(node.bytes))
            })
            .count();
        let record_count = u64::try_from(record_count).map_err(|_| {
            IdentityValidationError::Resource(WireError::new(
                WireErrorKind::IntegerOutOfRange,
                path.clone(),
                None,
            ))
        })?;
        if matches!(scope, RecordScope::Closure) {
            meter
                .charge_work(
                    record_count.saturating_mul(u64::from(record_count.max(1).ilog2()) + 1),
                    path,
                )
                .map_err(IdentityValidationError::Resource)?;
        }
        let mut records = Vec::new();
        meter
            .try_reserve_exact(&mut records, record_count, COLLECTION_ELEMENT_BYTES, path)
            .map_err(IdentityValidationError::Resource)?;
        for (node, candidate) in &self.candidates {
            if node.kind != I::KIND || !scope.includes(candidate.layer) {
                continue;
            }
            let slot = CanonicalKeySlot::new::<I, K>(node.bytes);
            let Some(key) = self.canonical_keys.get(&slot).cloned() else {
                continue;
            };
            let Ok(key) = key.into_any().downcast::<K>() else {
                return Err(IdentityValidationError::InvalidRecord {
                    kind: I::KIND,
                    id: node.bytes,
                    reason: "validated identity has the wrong concrete key type".to_owned(),
                });
            };
            let Some(id) = candidate.trusted_id.downcast_ref::<I>().copied() else {
                return Err(IdentityValidationError::InvalidRecord {
                    kind: I::KIND,
                    id: node.bytes,
                    reason: "validated identity has the wrong concrete id type".to_owned(),
                });
            };
            let record = CborIdentityRecord::from_verified_shared(id, key);
            records.push(record);
        }
        records.sort_by_key(CborIdentityRecord::id);
        Ok(records)
    }
}
