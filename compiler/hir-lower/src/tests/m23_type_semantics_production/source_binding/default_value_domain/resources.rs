use super::*;

#[test]
fn default_value_domain_binding_charges_empty_and_nonempty_transactions() {
    for source in [SOURCE, "public class Empty {}"] {
        with_inputs(source, |inputs| {
            inputs.with_bound(|domains, parameters, _| {
                let declarations = parameters
                    .bind_default_declarations(&inputs.templates, &[], &mut meter())
                    .unwrap();
                let mut measured = meter();
                domains
                    .bind_nominal_default_value_domains(&declarations, &mut measured)
                    .unwrap();
                let work = measured.usage().validation_work_units;
                assert!(work > 0);
                let mut shared = BudgetMeter::new(DecodeLimits {
                    validation_work_units: work * 2 - 1,
                    ..DecodeLimits::default()
                });
                domains
                    .bind_nominal_default_value_domains(&declarations, &mut shared)
                    .unwrap();
                assert!(matches!(
                    domains.bind_nominal_default_value_domains(&declarations, &mut shared),
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
                ] {
                    assert!(matches!(
                        domains.bind_nominal_default_value_domains(
                            &declarations,
                            &mut BudgetMeter::new(limits)
                        ),
                        Err(Error::Resource(_))
                    ));
                }
                for declaration in declarations.declarations() {
                    for occurrence in declaration.references().occurrences() {
                        let Some(target) = target(occurrence.source()) else {
                            continue;
                        };
                        let mut measured = meter();
                        domains.value_source_domain(target, &mut measured).unwrap();
                        let mut shared = BudgetMeter::new(DecodeLimits {
                            validation_work_units: measured.usage().validation_work_units * 2 - 1,
                            ..DecodeLimits::default()
                        });
                        domains.value_source_domain(target, &mut shared).unwrap();
                        assert!(matches!(
                            domains.value_source_domain(target, &mut shared),
                            Err(hir::DefaultSourceDomainError::Resource(_))
                        ));
                    }
                }
            });
        });
    }
}
