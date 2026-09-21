use super::*;

#[test]
fn default_type_binding_shares_resource_budget_across_the_complete_transaction() {
    for source in [SOURCE, COMBINATIONS] {
        with_inputs(source, |inputs| {
            inputs.with_bound(|domains, parameters, _| {
                let bound = parameters
                    .bind_default_declarations(&inputs.templates, &[], &mut meter())
                    .unwrap();
                let mut measured = meter();
                domains
                    .bind_nominal_default_type_domains(&bound, &mut measured)
                    .unwrap();
                let mut shared = BudgetMeter::new(DecodeLimits {
                    validation_work_units: measured.usage().validation_work_units * 2 - 1,
                    ..DecodeLimits::default()
                });
                domains
                    .bind_nominal_default_type_domains(&bound, &mut shared)
                    .unwrap();
                assert!(matches!(
                    domains.bind_nominal_default_type_domains(&bound, &mut shared),
                    Err(Error::Resource(_))
                ));
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
                        decoded_edges: 0,
                        ..DecodeLimits::default()
                    },
                    DecodeLimits {
                        semantic_recursion: 0,
                        ..DecodeLimits::default()
                    },
                    DecodeLimits {
                        semantic_table_entries: 0,
                        ..DecodeLimits::default()
                    },
                    DecodeLimits {
                        logical_heap_bytes: 0,
                        ..DecodeLimits::default()
                    },
                    DecodeLimits {
                        semantic_leaf_bytes: 0,
                        ..DecodeLimits::default()
                    },
                    DecodeLimits {
                        owned_bytes: 0,
                        ..DecodeLimits::default()
                    },
                ] {
                    assert!(
                        matches!(
                            domains.bind_nominal_default_type_domains(
                                &bound,
                                &mut BudgetMeter::new(limits)
                            ),
                            Err(Error::Resource(_))
                        ),
                        "{limits:?}"
                    );
                }
            })
        });
    }
}

#[test]
fn default_type_binding_charges_empty_templates_and_reference_free_defaults() {
    for source in [
        "public class Empty {}",
        "public class Empty<T>(seed: T, val value: T = seed)",
    ] {
        with_inputs(source, |inputs| {
            inputs.with_bound(|domains, parameters, _| {
                let bound = parameters
                    .bind_default_declarations(&inputs.templates, &[], &mut meter())
                    .unwrap();
                assert!(
                    bound
                        .declarations()
                        .iter()
                        .all(|d| d.references().occurrences().is_empty())
                );
                domains
                    .bind_nominal_default_type_domains(&bound, &mut meter())
                    .unwrap();
                let limits = DecodeLimits {
                    validation_work_units: 0,
                    ..DecodeLimits::default()
                };
                assert!(matches!(
                    domains
                        .bind_nominal_default_type_domains(&bound, &mut BudgetMeter::new(limits)),
                    Err(Error::Resource(_))
                ));
            })
        });
    }
}
