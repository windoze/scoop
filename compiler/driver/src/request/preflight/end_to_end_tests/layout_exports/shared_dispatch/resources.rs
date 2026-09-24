use super::*;

pub(super) fn check(input: LayoutAbiExportInputV1<'_>, abis: SharedLirDispatchAbiInputsV1<'_>) {
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
            semantic_table_entries: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            replay(input, abis, &mut BudgetMeter::new(limits)),
            Err(Error::Resource(_))
        ));
    }
    let mut measured = meter();
    replay(input, abis, &mut measured).unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    replay(input, abis, &mut shared).unwrap();
    assert!(replay(input, abis, &mut shared).is_err());
}
