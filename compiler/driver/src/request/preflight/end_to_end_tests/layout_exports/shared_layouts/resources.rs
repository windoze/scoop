use super::*;

pub(super) fn check(input: LayoutAbiExportInputV1<'_>) {
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
            semantic_recursion: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            replay(
                input,
                input.bridge.types(),
                &[],
                &mut BudgetMeter::new(limits)
            ),
            Err(Error::Resource(_))
        ));
    }
    let mut measured = meter();
    replay(input, input.bridge.types(), &[], &mut measured).unwrap();
    let limit = measured.usage().validation_work_units;
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: limit,
        ..DecodeLimits::default()
    });
    replay(input, input.bridge.types(), &[], &mut shared).unwrap();
    assert!(matches!(
        replay(input, input.bridge.types(), &[], &mut shared),
        Err(Error::Resource(_))
    ));
}
