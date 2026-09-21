use super::*;

#[test]
fn default_access_binding_charges_one_budget_through_identity_and_access_replay() {
    for source in [SOURCE, OUTSIDE_ROOTS, COMBINATIONS] {
        with_sources(source, |_, fixture, required, table| {
            let foundation = fixture.bind().unwrap();
            for limits in [
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
                DecodeLimits {
                    semantic_table_entries: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    semantic_recursion: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    semantic_leaf_bytes: 0,
                    ..DecodeLimits::default()
                },
            ] {
                assert!(
                    matches!(
                        foundation.bind_default_access_declarations(
                            table,
                            required,
                            &mut BudgetMeter::new(limits)
                        ),
                        Err(Error::Resource(_))
                    ),
                    "{limits:?}"
                );
            }
            let mut measured = meter();
            foundation
                .bind_default_access_declarations(table, required, &mut measured)
                .unwrap();
            let work = measured.usage().validation_work_units;
            let mut shared = BudgetMeter::new(DecodeLimits {
                validation_work_units: work * 2 - 1,
                ..DecodeLimits::default()
            });
            foundation
                .bind_default_access_declarations(table, required, &mut shared)
                .unwrap();
            assert!(matches!(
                foundation.bind_default_access_declarations(table, required, &mut shared),
                Err(Error::Resource(_))
            ));
        });
    }
}
