use super::*;

#[test]
fn complete_profile_transaction_and_queries_preserve_the_callers_budget() {
    for source in [STANDALONE, COMBINED, DOMAINS, "public class Empty {}"] {
        with_inputs(source, |inputs| {
            inputs.with_bound(|domains, parameters, _| {
                let declarations = parameters
                    .bind_default_declarations(inputs.production.templates(), &[], &mut meter())
                    .unwrap();
                let run = |meter: &mut BudgetMeter| {
                    domains
                        .bind_nominal_default_target_domains(&declarations, meter)
                        .unwrap()
                        .bind_source_profiles(inputs.production.profiles(), meter)
                };
                let mut measured = meter();
                let profiles = run(&mut measured).unwrap();
                let mut short = BudgetMeter::new(DecodeLimits {
                    validation_work_units: measured.usage().validation_work_units - 1,
                    ..DecodeLimits::default()
                });
                assert!(matches!(run(&mut short), Err(Error::Resource(_))));
                for record in profiles.profiles().records() {
                    let mut query = meter();
                    profiles
                        .default_access_profile(record.key(), &mut query)
                        .unwrap();
                    let mut shared = BudgetMeter::new(DecodeLimits {
                        validation_work_units: query.usage().validation_work_units * 2 - 1,
                        ..DecodeLimits::default()
                    });
                    profiles
                        .default_access_profile(record.key(), &mut shared)
                        .unwrap();
                    assert!(matches!(
                        profiles.default_access_profile(record.key(), &mut shared),
                        Err(Error::Resource(_))
                    ));
                }
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
                    let target = domains
                        .bind_nominal_default_target_domains(&declarations, &mut meter())
                        .unwrap();
                    assert!(matches!(
                        target.bind_source_profiles(
                            inputs.production.profiles(),
                            &mut BudgetMeter::new(limits)
                        ),
                        Err(Error::Resource(_))
                    ));
                }
            })
        });
    }
}
