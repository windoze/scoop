use super::*;

#[test]
fn callable_and_empty_constructor_operations_share_work_and_allocation_budgets() {
    for name in ["method", "empty"] {
        with_core_source(SOURCE, |output, bound, _| {
            let template = template(output, name, u32::from(name == "method"));
            let run = |meter: &mut BudgetMeter| {
                if name == "method" {
                    bound
                        .members()
                        .default_member_callable_shape(
                            &callables(&template)[0],
                            meter,
                            &WirePath::root(),
                        )
                        .map(|_| ())
                } else {
                    bound
                        .default_constructor_operation_shape(
                            &constructor(&template),
                            meter,
                            &WirePath::root(),
                        )
                        .map(|_| ())
                }
            };
            let mut measured = meter();
            run(&mut measured).unwrap();
            let mut shared = BudgetMeter::new(DecodeLimits {
                validation_work_units: measured.usage().validation_work_units * 2 - 1,
                ..DecodeLimits::default()
            });
            run(&mut shared).unwrap();
            assert!(matches!(run(&mut shared), Err(Error::Resource(_))));
            for limits in [
                DecodeLimits {
                    decoded_nodes: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    semantic_recursion: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    logical_heap_bytes: 0,
                    ..DecodeLimits::default()
                },
            ] {
                let result = run(&mut BudgetMeter::new(limits));
                assert!(
                    matches!(result, Err(Error::Resource(_))),
                    "{name}: {limits:?}: {result:?}"
                );
            }
        });
    }
}
