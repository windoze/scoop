use super::*;

#[test]
fn nominal_operation_queries_charge_empty_and_substituted_shapes_to_one_budget() {
    for (text, name, position) in [
        (SOURCE, "empty", 0),
        (SOURCE, "packet", 0),
        (COMBINED, "pair", 1),
    ] {
        with_source(text, |output, nominals| {
            let ty = value_type(output, name, position);
            let mut measured = meter();
            nominals
                .default_nominal_operation_shape(
                    Target::Struct(&ty),
                    &mut measured,
                    &WirePath::root(),
                )
                .unwrap();
            let work = measured.usage().validation_work_units;
            let mut shared = BudgetMeter::new(DecodeLimits {
                validation_work_units: work * 2 - 1,
                ..DecodeLimits::default()
            });
            nominals
                .default_nominal_operation_shape(
                    Target::Struct(&ty),
                    &mut shared,
                    &WirePath::root(),
                )
                .unwrap();
            assert!(matches!(
                nominals.default_nominal_operation_shape(
                    Target::Struct(&ty),
                    &mut shared,
                    &WirePath::root()
                ),
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
                DecodeLimits {
                    logical_heap_bytes: 0,
                    ..DecodeLimits::default()
                },
            ] {
                let result = nominals.default_nominal_operation_shape(
                    Target::Struct(&ty),
                    &mut BudgetMeter::new(limits),
                    &WirePath::root(),
                );
                assert!(
                    matches!(result, Err(Error::Resource(_))),
                    "{name}, {limits:?}: {result:?}"
                );
            }
        });
    }
}
