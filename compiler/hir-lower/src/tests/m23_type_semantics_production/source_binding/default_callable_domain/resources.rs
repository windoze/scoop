use super::*;

#[test]
fn callable_domain_binding_preserves_shared_budget_for_empty_and_complete_defaults() {
    for source in [
        SOURCE,
        "public class Empty {}",
        "public class Literal(val value: Int = 1)",
    ] {
        with_inputs(source, |inputs| {
            inputs.with_bound(|domains, parameters, _| {
                let declarations = parameters
                    .bind_default_declarations(&inputs.templates, &[], &mut meter())
                    .unwrap();
                let mut measured = meter();
                domains
                    .bind_nominal_default_callable_domains(&declarations, &mut measured)
                    .unwrap();
                let work = measured.usage().validation_work_units;
                assert!(work > 0);
                let mut shared = BudgetMeter::new(DecodeLimits {
                    validation_work_units: work * 2 - 1,
                    ..DecodeLimits::default()
                });
                domains
                    .bind_nominal_default_callable_domains(&declarations, &mut shared)
                    .unwrap();
                assert!(matches!(
                    domains.bind_nominal_default_callable_domains(&declarations, &mut shared),
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
                        semantic_recursion: 0,
                        ..DecodeLimits::default()
                    },
                ] {
                    assert!(matches!(
                        domains.bind_nominal_default_callable_domains(
                            &declarations,
                            &mut BudgetMeter::new(limits)
                        ),
                        Err(Error::Resource(_))
                    ));
                }
            });
        });
    }
}
