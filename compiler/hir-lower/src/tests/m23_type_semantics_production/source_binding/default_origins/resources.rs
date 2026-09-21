use super::*;

#[test]
fn default_location_binding_uses_the_callers_remaining_budget() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let table = templates(output);
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let parameters = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            for limits in [
                DecodeLimits {
                    semantic_table_entries: 0,
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
                DecodeLimits {
                    semantic_leaf_bytes: 0,
                    ..DecodeLimits::default()
                },
            ] {
                assert!(
                    parameters
                        .bind_default_origins(&table, &[], &mut BudgetMeter::new(limits))
                        .is_err(),
                    "{limits:?}"
                );
            }
            let mut measured = meter();
            parameters
                .bind_default_origins(&table, &[], &mut measured)
                .unwrap();
            let work = measured.usage().validation_work_units;
            let mut shared = BudgetMeter::new(DecodeLimits {
                validation_work_units: work * 2 - 1,
                ..DecodeLimits::default()
            });
            parameters
                .bind_default_origins(&table, &[], &mut shared)
                .unwrap();
            assert!(
                parameters
                    .bind_default_origins(&table, &[], &mut shared)
                    .is_err()
            );
        });
    });
}
