use super::*;

macro_rules! borrowed_index {
    ($name:ident, $lookup:ident, $table:ty, $record:ty, $key:ty, $records:ident, $key_fn:ident, $error:ident, $field:ident, $merge:expr) => {
        /// An index that borrows records without changing export tables.
        #[derive(Debug)]
        pub struct $name<'a> {
            records: Vec<&'a $record>,
        }
        impl<'a> $name<'a> {
            pub fn try_new(tables: &[&'a $table]) -> Result<Self, MirTypeBridgeLookupError> {
                let path = WirePath::root();

                let count = tables.iter().try_fold(0usize, |count, table| {
                    count
                        .checked_add(table.$records().len())
                        .ok_or(MirTypeBridgeLookupError::RecordCountOverflow)
                })?;

                let mut records: Vec<&'a $record> = Vec::new();
                scoop_wire::allocation::try_reserve(&mut records, count, &path)?;
                for table in tables {
                    records.extend(table.$records());
                }
                records.sort_unstable_by_key(|record| record.$key_fn());

                if let Some(pair) = records.windows(2).find(|pair| {
                    pair[0].$key_fn() == pair[1].$key_fn() && !($merge)(pair[0], pair[1])
                }) {
                    return Err(MirTypeBridgeLookupError::$error {
                        $field: pair[0].$key_fn(),
                    });
                }
                records.dedup_by_key(|record| record.$key_fn());
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
    exact,
    |left: &ParamFreeMirTypeExportV1, right: &ParamFreeMirTypeExportV1| left.is_odr()
        && left == right
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
    owner,
    |left: &ParamFreeMirDispatchSchemaV1, right: &ParamFreeMirDispatchSchemaV1| left.is_odr()
        && left == right
);
