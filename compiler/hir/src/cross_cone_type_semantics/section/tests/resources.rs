use super::*;
use std::sync::Arc;

struct Counting {
    graph: ValidatedIdentityGraph,
    calls: usize,
}
impl<I: PersistentId> PersistentIdResolver<I> for Counting
where
    ValidatedIdentityGraph: PersistentIdResolver<I, Error = IdentityReferenceError>,
{
    type Error = IdentityReferenceError;
    fn resolve(&mut self, id: DecodedPersistentId<I>) -> Result<I, Self::Error> {
        self.calls += 1;
        self.graph.resolve(id)
    }
}
impl<I: PersistentId, K> PersistentKeyResolver<I, K> for Counting
where
    ValidatedIdentityGraph: PersistentKeyResolver<I, K, Error = IdentityReferenceError>,
{
    type Error = IdentityReferenceError;
    fn resolve_key(&mut self, id: DecodedPersistentId<I>) -> Result<Arc<K>, Self::Error> {
        self.calls += 1;
        self.graph.resolve_key(id)
    }
}

#[test]
fn section_checks_outer_and_fact_resources_before_resolver_callbacks() {
    let fixture = Fixture::new();
    let section = fixture.section();
    for limits in [
        DecodeLimits {
            decoded_nodes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_table_entries: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_leaf_bytes: 31,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_recursion: 1,
            ..DecodeLimits::default()
        },
    ] {
        let mut resolver = Counting {
            graph: fixture.resolver(),
            calls: 0,
        };
        let error = decoded(&section).resolve(
            &mut resolver,
            &mut BudgetMeter::new(limits),
            &WirePath::root().field(42),
        );
        assert!(matches!(
            error,
            Err(TypeSemanticsSectionResolutionError::Resource(_))
                | Err(TypeSemanticsSectionResolutionError::Facts(
                    MeteredExactTypeFactsResolutionError::Resource(_)
                ))
        ));
        assert_eq!(resolver.calls, 0);
    }
}

#[test]
fn one_meter_accumulates_across_all_eight_tables() {
    let fixture = Fixture::new();
    let section = fixture.section();
    let mut complete = meter();
    decoded(&section)
        .resolve(&mut fixture.resolver(), &mut complete, &WirePath::root())
        .unwrap();
    let limits = DecodeLimits {
        validation_work_units: complete.usage().validation_work_units - 1,
        ..DecodeLimits::default()
    };
    let error = decoded(&section)
        .resolve(
            &mut fixture.resolver(),
            &mut BudgetMeter::new(limits),
            &WirePath::root().field(42),
        )
        .unwrap_err();
    assert!(
        matches!(error, TypeSemanticsSectionResolutionError::Selected(SelectedTypeUseResolutionError::Resource(ref error))
        if matches!(error.kind(), WireErrorKind::LimitExceeded { resource: ResourceKind::ValidationWorkUnits, .. }))
    );
}

#[test]
fn facts_budget_reports_the_aggregate_field_path_without_encoding_a_product_as_a_leaf() {
    let fixture = Fixture::new();
    let section = fixture.section();
    let error = decoded(&section)
        .resolve(
            &mut fixture.resolver(),
            &mut BudgetMeter::new(DecodeLimits {
                semantic_leaf_bytes: 31,
                ..DecodeLimits::default()
            }),
            &WirePath::root().field(42),
        )
        .unwrap_err();
    let TypeSemanticsSectionResolutionError::Facts(MeteredExactTypeFactsResolutionError::Resource(
        error,
    )) = error
    else {
        panic!("wrong phase");
    };
    assert_eq!(
        error.path(),
        &WirePath::root().field(42).field(1).index(0).field(1)
    );
    // The record exceeds 32 encoded bytes, while each semantic leaf fits.
    let decoded: DecodedCanonicalExactTypeFactsV1 = decode_canonical(
        &encode(section.exact_facts()).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    assert!(
        decoded
            .resolve_metered(
                &mut fixture.resolver(),
                &mut BudgetMeter::new(DecodeLimits {
                    semantic_leaf_bytes: 32,
                    ..DecodeLimits::default()
                }),
                &WirePath::root()
            )
            .is_ok()
    );
}
