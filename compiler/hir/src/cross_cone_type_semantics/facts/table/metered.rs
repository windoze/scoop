use super::*;
use scoop_wire::{BudgetMeter, WirePath};

impl DecodedCanonicalExactTypeFactsV1 {
    /// Resolve fixed-size facts with the section's shared resource meter.
    /// Canonical order is checked as received and is never repaired.
    pub fn resolve_metered<R: PersistentIdResolver<PersistentExactTypeId>>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<CanonicalExactTypeFactsV1, MeteredExactTypeFactsResolutionError<R::Error>> {
        use MeteredExactTypeFactsResolutionError as Error;
        let mut records = Vec::new();
        meter.check_table_entries(self.records.len() as u64, path)?;
        meter.charge_work(self.records.len() as u64, path)?;
        meter.try_reserve_collection_slots(&mut records, self.records.len(), path)?;
        for (index, decoded) in self.records.into_iter().enumerate() {
            let at = path.clone().index(index as u64);
            meter.check_semantic_depth(1, &at)?;
            meter.charge_nodes(1, &at)?;
            meter.check_semantic_leaf(32, &at.clone().field(1))?;
            meter.charge_edges(1, &at.clone().field(1))?;
            meter.charge_work(32, &at.clone().field(1))?;
            // Kind and GC are closed scalar tags; Value has one nested ZST tag.
            meter.check_semantic_depth(2, &at.clone().field(2))?;
            if matches!(decoded.kind, super::super::ExactTypeKindV1::Value { .. }) {
                meter.check_semantic_depth(3, &at.clone().field(2).field(1))?;
                meter.charge_nodes(1, &at.clone().field(2).field(1))?;
            }
            meter.charge_nodes(2, &at)?;
            let record = decoded.resolve(resolver).map_err(|error| {
                Error::Semantic(ExactTypeFactsTableResolutionError::Record { index, error })
            })?;
            if records
                .last()
                .is_some_and(|previous: &ExactTypeFactsV1| previous.exact() >= record.exact())
            {
                return Err(Error::Semantic(ExactTypeFactsTableResolutionError::Order(
                    ExactTypeFactsTableError {
                        index,
                        exact: record.exact(),
                    },
                )));
            }
            records.push(record);
        }
        Ok(CanonicalExactTypeFactsV1 { records })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum MeteredExactTypeFactsResolutionError<E> {
    Resource(WireError),
    Semantic(ExactTypeFactsTableResolutionError<E>),
}
impl<E> From<WireError> for MeteredExactTypeFactsResolutionError<E> {
    fn from(value: WireError) -> Self {
        Self::Resource(value)
    }
}
impl<E: fmt::Display> fmt::Display for MeteredExactTypeFactsResolutionError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Semantic(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for MeteredExactTypeFactsResolutionError<E> {}
