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
                    .bind_parameter_protocols(constructors, &sources.protocols)
                    .unwrap();
                let bound = parameters.bind_default_declarations(&table, &[]).unwrap();
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
                                .lookup(site, occurrence.descriptor().identity())
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
                    &[
                        (
                            "NestedIndexHost.bound",
                            1,
                            &[Kind::Lambda, Kind::CallableReference],
                        ),
                        (
                            "NestedIndexHost.local",
                            1,
                            &[Kind::LocalFunction, Kind::Lambda, Kind::Lambda],
                        ),
                    ]
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
                    let contract = bound.declaration(key(output, name, position)).unwrap();
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
        let index = template.index_nested_callables(&path).unwrap();
        let identity = index.occurrences()[0].descriptor().identity();
        let Identity::CallableReference(invoke) = identity else {
            panic!("expected invoke");
        };
        assert!(matches!(
            index.lookup(Site::Standalone, identity),
            Err(QueryError::Standalone)
        ));
        for ordinal in [2, u64::MAX] {
            assert!(
                matches!(index.lookup(Site::Body { ordinal }, identity), Err(QueryError::Missing { ordinal: actual }) if actual == ordinal)
            );
        }
        assert!(matches!(
            index.lookup(Site::Body { ordinal: 0 }, Identity::Lambda(invoke)),
            Err(QueryError::Identity { expected, actual: Identity::Lambda(actual), .. }) if expected == identity && actual == invoke
        ));

        index.lookup(Site::Body { ordinal: 0 }, identity).unwrap();
    });
}
