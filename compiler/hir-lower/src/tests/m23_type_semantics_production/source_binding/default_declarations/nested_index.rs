use super::*;
use crate::tests::m23_type_semantics_production::source_dispatch::with_hir_source;
use hir::{
    DefaultNestedCallableIdentityV1 as Identity, DefaultNestedCallableKindV1 as Kind,
    DefaultNestedCallableSiteV1 as Site, DefaultSourceNestedCallableQueryError as QueryError,
};
use scoop_wire::WirePath;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/nested-index.scoop"
));
const REPEATED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/nested-occurrences.scoop"
));

#[test]
fn bound_source_nested_index_preserves_descriptor_preorder_and_lowered_receivers() {
    for source in [SOURCE, super::nested_identities::SOURCE] {
        with_sources(source, |output, fixture, sources, core| {
            let table = templates(output);
            let foundation = fixture.bind().unwrap();
            sources.with_bound(&foundation, core, |members, constructors| {
                let parameters = members
                    .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                    .unwrap();
                let bound = parameters
                    .bind_default_declarations(&table, &[], &mut meter())
                    .unwrap();
                for contract in bound.declarations() {
                    let index = contract.nested_callables();
                    assert_eq!(index.template(), contract.key());
                    for (ordinal, occurrence) in index.occurrences().iter().enumerate() {
                        let site = Site::Body {
                            ordinal: ordinal as u64,
                        };
                        assert_eq!(occurrence.site(), site);
                        assert!(std::ptr::eq(
                            index
                                .lookup(
                                    site,
                                    occurrence.descriptor().identity(),
                                    &mut meter(),
                                    &WirePath::root()
                                )
                                .unwrap(),
                            occurrence,
                        ));
                        assert_eq!(
                            occurrence.definition_origin().origin().source().cone(),
                            bound.provider()
                        );
                    }
                }
                let expected: &[(&str, u32, &[Kind])] = if source == SOURCE {
                    &[(
                        "NestedIndexHost.bound",
                        1,
                        &[Kind::Lambda, Kind::CallableReference],
                    )]
                } else {
                    &[
                        ("NestedIdentityHost.lambda", 1, &[Kind::Lambda]),
                        (
                            "NestedIdentityHost.anonymous",
                            1,
                            &[Kind::AnonymousFunction],
                        ),
                        (
                            "NestedIdentityHost.reference",
                            0,
                            &[Kind::CallableReference],
                        ),
                        (
                            "NestedIdentityHost.local",
                            1,
                            &[Kind::LocalFunction, Kind::CallableReference, Kind::Lambda],
                        ),
                    ]
                };
                for &(name, position, kinds) in expected {
                    let contract = bound
                        .declaration(key(output, name, position), &mut meter())
                        .unwrap();
                    let occurrences = contract.nested_callables().occurrences();
                    assert_eq!(
                        occurrences
                            .iter()
                            .map(|o| o.descriptor().identity().kind())
                            .collect::<Vec<_>>(),
                        kinds,
                        "{name}"
                    );
                }
            });
        });
    }
}

#[test]
fn source_nested_index_rejects_unlocated_missing_and_wrong_identity_queries() {
    with_hir_source(REPEATED, |output, _| {
        let template = source_template(output, "OccurrenceHost.pair", 0);
        let path = WirePath::root().field(4);
        let index = template
            .index_nested_callables(&mut meter(), &path)
            .unwrap();
        let identity = index.occurrences()[0].descriptor().identity();
        let Identity::CallableReference(invoke) = identity else {
            panic!("expected invoke");
        };
        assert!(matches!(
            index.lookup(Site::Standalone, identity, &mut meter(), &path),
            Err(QueryError::Standalone)
        ));
        for ordinal in [2, u64::MAX] {
            assert!(
                matches!(index.lookup(Site::Body { ordinal }, identity, &mut meter(), &path), Err(QueryError::Missing { ordinal: actual }) if actual == ordinal)
            );
        }
        assert!(matches!(
            index.lookup(Site::Body { ordinal: 0 }, Identity::Lambda(invoke), &mut meter(), &path),
            Err(QueryError::Identity { expected, actual: Identity::Lambda(actual), .. }) if expected == identity && actual == invoke
        ));
        let mut measured = meter();
        index
            .lookup(Site::Body { ordinal: 0 }, identity, &mut measured, &path)
            .unwrap();
        let mut shared = BudgetMeter::new(DecodeLimits {
            validation_work_units: measured.usage().validation_work_units * 2 - 1,
            ..DecodeLimits::default()
        });
        index
            .lookup(Site::Body { ordinal: 0 }, identity, &mut shared, &path)
            .unwrap();
        assert!(matches!(
            index.lookup(Site::Body { ordinal: 1 }, identity, &mut shared, &path),
            Err(QueryError::Resource(_))
        ));
    });
}

#[test]
fn source_nested_index_uses_shared_construction_limits() {
    with_hir_source(REPEATED, |output, _| {
        let template = source_template(output, "OccurrenceHost.pair", 0);
        let path = WirePath::root().field(4);
        for limits in [
            DecodeLimits {
                validation_work_units: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_recursion: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_table_entries: 1,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                owned_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(
                template
                    .index_nested_callables(&mut BudgetMeter::new(limits), &path)
                    .is_err(),
                "{limits:?}"
            );
        }
        let mut measured = meter();
        template
            .index_nested_callables(&mut measured, &path)
            .unwrap();
        let mut shared = BudgetMeter::new(DecodeLimits {
            validation_work_units: measured.usage().validation_work_units * 2 - 1,
            ..DecodeLimits::default()
        });
        template.index_nested_callables(&mut shared, &path).unwrap();
        assert!(template.index_nested_callables(&mut shared, &path).is_err());
    });
}
