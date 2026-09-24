use super::*;

pub(super) fn check(
    input: LayoutAbiExportInputV1<'_>,
    expected: &lir::LayoutAbiExportConstituentsV1,
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
        assert!(
            replay(
                input,
                expected.layouts(),
                expected.dispatch(),
                &[],
                &mut BudgetMeter::new(limits)
            )
            .is_err()
        );
    }
    let mut measured = meter();
    replay(
        input,
        expected.layouts(),
        expected.dispatch(),
        &[],
        &mut measured,
    )
    .unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    replay(
        input,
        expected.layouts(),
        expected.dispatch(),
        &[],
        &mut shared,
    )
    .unwrap();
    assert!(
        replay(
            input,
            expected.layouts(),
            expected.dispatch(),
            &[],
            &mut shared
        )
        .is_err()
    );
}
