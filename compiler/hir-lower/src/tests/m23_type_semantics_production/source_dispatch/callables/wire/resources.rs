use super::*;
use scoop_identity::{
    DecodedPersistentId, PersistentId, PersistentIdResolver, PersistentKeyResolver,
};
use std::sync::Arc;

#[derive(Default)]
struct NoQueries(usize);
impl<I: PersistentId> PersistentIdResolver<I> for NoQueries {
    type Error = &'static str;
    fn resolve(&mut self, _: DecodedPersistentId<I>) -> Result<I, Self::Error> {
        self.0 += 1;
        Err("resource limit must precede reference lookup")
    }
}
impl<I: PersistentId, K> PersistentKeyResolver<I, K> for NoQueries {
    type Error = &'static str;
    fn resolve_key(&mut self, _: DecodedPersistentId<I>) -> Result<Arc<K>, Self::Error> {
        self.0 += 1;
        Err("resource limit must precede key lookup")
    }
}

#[test]
fn source_callable_resolution_accounts_for_nested_access_before_querying_identities() {
    with_hir_source(CALLABLES, |output, _| {
        let bytes = encode(&table(output)).unwrap();
        for limits in [
            DecodeLimits {
                semantic_recursion: 3,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
        ] {
            let decoded: DecodedTable = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
            let mut resolver = NoQueries::default();
            assert!(matches!(
                decoded.resolve(&mut resolver, &mut BudgetMeter::new(limits)),
                Err(Error::Resource(_))
                    | Err(Error::Inventory(hir::SourceInventoryError::Resource(_)))
            ));
            assert_eq!(resolver.0, 0);
        }
    });
}
