use super::*;

pub(super) fn check(
    replay: impl Fn(&mut BudgetMeter) -> Result<lir::CrossConeLirBridgeSectionV1, Error>,
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
        assert!(replay(&mut BudgetMeter::new(limits)).is_err());
    }
    let mut measured = meter();
    let expected = replay(&mut measured).unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    replay(&mut shared).unwrap();
    assert!(replay(&mut shared).is_err());
    let wire: lir::DecodedCrossConeLirBridgeSectionV1 = decoded(&expected);
    assert!(
        wire.validate_against(
            expected,
            &mut BudgetMeter::new(DecodeLimits {
                logical_heap_bytes: 0,
                ..DecodeLimits::default()
            })
        )
        .is_err()
    );
}
