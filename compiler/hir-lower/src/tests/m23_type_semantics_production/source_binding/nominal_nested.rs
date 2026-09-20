use super::super::source_dispatch::with_hir_source;
use super::nominal_parameters::support::Sources;
use super::*;
use hir::{
    NestedSourceSupportV1 as Entry, NominalNestedBindingError as Error,
    NominalSupportNestedInterfaceV1 as Record,
};
use scoop_identity::{CallableTemplateOrigin, ExactTypeKey, PersistentExactTypeId};

mod corruption;
mod nominals;
mod protocols;
mod support;
use support::*;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/nested-binding.scoop"
));

#[test]
fn restored_nested_sources_replay_complete_recursive_contracts_and_protocols() {
    for input in [
        SOURCE,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/nested.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/parameters-binding.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/properties.scoop"
        )),
    ] {
        with_candidates(input, |fixture, sources, candidates, core| {
            let foundation = fixture.bind().unwrap();
            sources.with_bound(&foundation, core, |members, constructors| {
                let mut authority = members
                    .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                    .unwrap();
                let reps = &fixture.source.entries().representations;
                for record in &candidates.records {
                    let checked = authority
                        .validate_nested_source(record, &candidates.protocols, reps, &mut meter())
                        .unwrap();
                    assert_eq!(checked.provider(), fixture.source.entries().provider);
                    assert!(std::ptr::eq(checked.members(), members));
                    assert!(std::ptr::eq(checked.constructors(), constructors));
                    assert!(std::ptr::eq(
                        checked.parameter_sources(),
                        &sources.protocols
                    ));
                    assert!(std::ptr::eq(checked.record(), record));
                    assert!(std::ptr::eq(checked.representations(), reps));
                    let mut expected = BTreeSet::new();
                    collect_protocols(record, &mut expected);
                    assert_eq!(
                        checked
                            .protocols()
                            .map(|p| p.record().owner())
                            .collect::<BTreeSet<_>>(),
                        expected
                    );
                    for owner in expected {
                        assert_eq!(
                            checked.protocol(owner).unwrap().record(),
                            candidates.protocols.get(owner).unwrap()
                        );
                    }
                }
                if input == SOURCE {
                    assert_eq!(
                        outline(fixture, &candidates.records),
                        include_str!(concat!(
                            env!("CARGO_MANIFEST_DIR"),
                            "/../../tests/fixtures/m23-type-source-nominals/nested-binding.snap"
                        ))
                    );
                }
            });
        });
    }
}

#[test]
fn nested_source_replay_stops_on_shared_resource_exhaustion() {
    with_candidates(SOURCE, |fixture, sources, candidates, core| {
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let mut authority = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            for limits in [
                DecodeLimits {
                    semantic_table_entries: 0,
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
                    decoded_nodes: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    semantic_recursion: 0,
                    ..DecodeLimits::default()
                },
            ] {
                assert!(
                    authority
                        .validate_nested_source(
                            &candidates.records[0],
                            &candidates.protocols,
                            &fixture.source.entries().representations,
                            &mut BudgetMeter::new(limits)
                        )
                        .is_err(),
                    "{limits:?}"
                );
            }
        });
    });
}
