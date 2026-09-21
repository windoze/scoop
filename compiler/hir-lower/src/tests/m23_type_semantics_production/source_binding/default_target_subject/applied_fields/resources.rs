use super::*;

#[test]
fn applied_default_field_queries_charge_shared_work_and_structural_budgets() {
    with_hir_source(COMBINATIONS, |output, _| {
        let fixture = Fixture::from_output(output);
        let foundation = fixture.bind().unwrap();
        for (name, position) in COMBINED_CASES {
            let record = reference(output, name, *position);
            let mut measured = meter();
            foundation
                .default_field_access_subject(record.target(), &mut measured)
                .unwrap();
            let mut shared = BudgetMeter::new(DecodeLimits {
                validation_work_units: measured.usage().validation_work_units * 2 - 1,
                ..DecodeLimits::default()
            });
            foundation
                .default_field_access_subject(record.target(), &mut shared)
                .unwrap();
            assert!(
                matches!(
                    foundation.default_field_access_subject(record.target(), &mut shared),
                    Err(Error::Resource(_))
                ),
                "{name}"
            );
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
            ] {
                assert!(
                    matches!(
                        foundation.default_field_access_subject(
                            record.target(),
                            &mut BudgetMeter::new(limits)
                        ),
                        Err(Error::Resource(_))
                    ),
                    "{name} {limits:?}"
                );
            }
            if !matches!(record.target(), Field::Tuple { .. }) {
                for limits in [
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
                ] {
                    assert!(
                        matches!(
                            foundation.default_field_access_subject(
                                record.target(),
                                &mut BudgetMeter::new(limits)
                            ),
                            Err(Error::Resource(_))
                        ),
                        "{name} {limits:?}"
                    );
                }
            }
        }
        // Classification preserves an unvalidated tuple position. Only an
        // operation contract can establish bounds and grant projection access.
        assert_eq!(
            foundation
                .default_field_access_subject(
                    &Field::Tuple {
                        declaration_index: u32::MAX
                    },
                    &mut meter()
                )
                .unwrap(),
            AccessSubject::TupleElement {
                declaration_index: u32::MAX
            }
        );
    });
}
