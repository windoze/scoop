use super::*;

#[test]
fn default_source_lookup_domain_replay_charges_the_callers_shared_budget() {
    for (source, name) in [
        (SOURCE, "fileOnly"),
        (COMBINATIONS, "GenericHost.Static.both"),
    ] {
        with_sources(source, |output, fixture, required, table| {
            let foundation = fixture.bind().unwrap();
            let bound = foundation
                .bind_default_access_declarations(table, required, &mut meter())
                .unwrap();
            let subject = function(output.output().export.module(), name);
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
            ] {
                assert!(
                    bound
                        .source_lookup_domain(subject, &mut BudgetMeter::new(limits))
                        .is_err(),
                    "{limits:?}"
                );
            }
            if name == "fileOnly" {
                assert!(
                    bound
                        .source_lookup_domain(
                            subject,
                            &mut BudgetMeter::new(DecodeLimits {
                                semantic_leaf_bytes: 0,
                                ..DecodeLimits::default()
                            })
                        )
                        .is_err()
                );
            }
            let mut measured = meter();
            bound.source_lookup_domain(subject, &mut measured).unwrap();
            let work = measured.usage().validation_work_units;
            let mut shared = BudgetMeter::new(DecodeLimits {
                validation_work_units: work * 2 - 1,
                ..DecodeLimits::default()
            });
            bound.source_lookup_domain(subject, &mut shared).unwrap();
            assert!(bound.source_lookup_domain(subject, &mut shared).is_err());
        });
    }
}
