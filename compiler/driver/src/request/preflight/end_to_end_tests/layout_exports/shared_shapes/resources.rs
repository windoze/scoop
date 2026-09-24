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
                expected.descriptors(),
                &mut BudgetMeter::new(limits)
            )
            .is_err()
        );
    }
    let mut measured = meter();
    replay(
        input,
        expected.layouts(),
        expected.descriptors(),
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
        expected.descriptors(),
        &mut shared,
    )
    .unwrap();
    assert!(
        replay(
            input,
            expected.layouts(),
            expected.descriptors(),
            &mut shared
        )
        .is_err()
    );
    let shapes = expected.shape_support();
    let wire: lir::DecodedCanonicalParamFreeShapeSupportExportsV1 = decoded(shapes);
    assert!(
        wire.validate_against(
            shapes,
            &mut BudgetMeter::new(DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            })
        )
        .is_err()
    );
}
