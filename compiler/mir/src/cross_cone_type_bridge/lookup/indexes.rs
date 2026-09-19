use super::*;

macro_rules! borrowed_index {
    ($name:ident, $lookup:ident, $table:ty, $record:ty, $key:ty, $records:ident, $key_fn:ident, $error:ident, $field:ident) => {
        /// A budgeted index that borrows records without changing export tables.
        #[derive(Debug)]
        pub struct $name<'a> {
            records: Vec<&'a $record>,
        }
        impl<'a> $name<'a> {
            pub fn try_new(
                tables: &[&'a $table],
                meter: &mut BudgetMeter,
            ) -> Result<Self, MirTypeBridgeLookupError> {
                let path = WirePath::root();
                meter.check_table_entries(tables.len() as u64, &path)?;
                meter.charge_work(tables.len() as u64, &path)?;
                let count = tables.iter().try_fold(0usize, |count, table| {
                    count
                        .checked_add(table.$records().len())
                        .ok_or(MirTypeBridgeLookupError::RecordCountOverflow)
                })?;
                meter.check_table_entries(count as u64, &path)?;
                meter.charge_nodes(count as u64, &path)?;
                let mut records: Vec<&'a $record> = Vec::new();
                meter.try_reserve_collection_slots(&mut records, count, &path)?;
                for table in tables {
                    records.extend(table.$records());
                }
                for _ in 0..usize::BITS - count.max(1).saturating_sub(1).leading_zeros() {
                    meter.charge_work(count as u64, &path)?;
                }
                records.sort_unstable_by_key(|record| record.$key_fn());
                meter.charge_work(count as u64, &path)?;
                if let Some(pair) = records
                    .windows(2)
                    .find(|pair| pair[0].$key_fn() == pair[1].$key_fn())
                {
                    return Err(MirTypeBridgeLookupError::$error {
                        $field: pair[0].$key_fn(),
                    });
                }
                Ok(Self { records })
            }
        }
        impl sealed::Sealed for $name<'_> {}
        impl $lookup for $name<'_> {
            fn get(&self, key: $key) -> Option<&$record> {
                self.records
                    .binary_search_by_key(&key, |record| record.$key_fn())
                    .ok()
                    .map(|index| self.records[index])
            }
            fn record_count(&self) -> usize {
                self.records.len()
            }
        }
    };
}

borrowed_index!(
    MirTypeBridgeTypeIndexV1,
    MirTypeBridgeTypeLookupV1,
    CanonicalParamFreeMirTypeExportsV1,
    ParamFreeMirTypeExportV1,
    PersistentExactTypeId,
    records,
    exact,
    DuplicateType,
    exact
);
borrowed_index!(
    MirTypeBridgeCallableIndexV1,
    MirTypeBridgeCallableLookupV1,
    CanonicalMirCallableBindingsV1,
    ParamFreeMirCallableBindingV1,
    StrongCallableDefinitionOwner,
    entries,
    implementation,
    DuplicateCallable,
    target
);
borrowed_index!(
    MirTypeBridgeSchemaIndexV1,
    MirTypeBridgeSchemaLookupV1,
    CanonicalMirDispatchSchemasV1,
    ParamFreeMirDispatchSchemaV1,
    PersistentExactTypeId,
    records,
    owner,
    DuplicateSchema,
    owner
);
