use super::*;

pub(super) fn check(
    input: LayoutAbiExportInputV1<'_>,
    layouts: &lir::CanonicalExactLayoutExportsV1,
) {
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
            replay(input, layouts, &[], &mut BudgetMeter::new(limits)),
            Err(Error::Resource(_))
        ));
    }
    let mut measured = meter();
    replay(input, layouts, &[], &mut measured).unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    replay(input, layouts, &[], &mut shared).unwrap();
    assert!(replay(input, layouts, &[], &mut shared).is_err());
}
