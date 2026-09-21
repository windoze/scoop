use super::*;

#[test]
fn default_callable_and_global_queries_share_resource_budgets() {
    for (source, cases) in [(SOURCE, CASES), (COMBINATIONS, COMBINED_CASES)] {
        with_hir_source(source, |output, _| {
            let fixture = Fixture::from_output(output);
            let foundation = fixture.bind().unwrap();
            for (name, position) in cases {
                let template = template(output, name, *position);
                for record in template.references().callables() {
                    let mut measured = meter();
                    foundation
                        .default_callable_access_subject(record.target(), &mut measured)
                        .unwrap();
                    let mut shared = BudgetMeter::new(DecodeLimits {
                        validation_work_units: measured.usage().validation_work_units * 2 - 1,
                        ..DecodeLimits::default()
                    });
                    foundation
                        .default_callable_access_subject(record.target(), &mut shared)
                        .unwrap();
                    assert!(
                        matches!(
                            foundation
                                .default_callable_access_subject(record.target(), &mut shared),
                            Err(Error::Resource(_))
                        ),
                        "{name}"
                    );
                    for limits in
                        limits(!matches!(record.target(), Callable::DerivedEquality { .. }))
                    {
                        assert!(
                            matches!(
                                foundation.default_callable_access_subject(
                                    record.target(),
                                    &mut BudgetMeter::new(limits)
                                ),
                                Err(Error::Resource(_))
                            ),
                            "{name} {limits:?}"
                        );
                    }
                }
                for record in template.references().globals() {
                    let mut measured = meter();
                    foundation
                        .default_global_access_subject(*record.target(), &mut measured)
                        .unwrap();
                    let mut shared = BudgetMeter::new(DecodeLimits {
                        validation_work_units: measured.usage().validation_work_units * 2 - 1,
                        ..DecodeLimits::default()
                    });
                    foundation
                        .default_global_access_subject(*record.target(), &mut shared)
                        .unwrap();
                    assert!(matches!(
                        foundation.default_global_access_subject(*record.target(), &mut shared),
                        Err(Error::Resource(_))
                    ));
                    for limits in limits(true) {
                        assert!(
                            matches!(
                                foundation.default_global_access_subject(
                                    *record.target(),
                                    &mut BudgetMeter::new(limits)
                                ),
                                Err(Error::Resource(_))
                            ),
                            "{name} {limits:?}"
                        );
                    }
                }
            }
        });
    }
}
fn limits(identity: bool) -> Vec<DecodeLimits> {
    let mut limits = vec![
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
    ];
    if identity {
        limits.extend([
            DecodeLimits {
                decoded_edges: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_leaf_bytes: 0,
                ..DecodeLimits::default()
            },
        ]);
    }
    limits
}
