use super::*;

pub(super) fn check(
    callables: &mir::StrongCallableBridgeSurfaceV1,
    layouts: &lir::CanonicalExactLayoutExportsV1,
    foundation: &lir::OdrFreeLirFoundation,
    replay: &impl Fn(
        &mir::StrongCallableBridgeSurfaceV1,
        &lir::CanonicalExactLayoutExportsV1,
        &mut BudgetMeter,
    ) -> Result<Option<Box<lir::CallableAbiRecordV1>>, Error>,
) {
    let role = callables.initialization_cycle().unwrap();
    let absent = mir::StrongCallableBridgeSurfaceV1::try_new(
        callables
            .bridges()
            .iter()
            .map(|callable| {
                mir::StrongCallableBridgeV1::new(
                    callable.implementation(),
                    callable.signature().clone(),
                )
            })
            .collect(),
    )
    .unwrap();
    assert!(replay(&absent, layouts, &mut meter()).unwrap().is_none());
    for exact in role
        .signature()
        .parameters()
        .iter()
        .copied()
        .chain([role.signature().result()])
    {
        let missing = lir::CanonicalExactLayoutExportsV1::try_new(
            layouts.target(),
            foundation,
            layouts
                .records()
                .iter()
                .filter(|record| record.identity().exact() != exact)
                .cloned()
                .collect(),
            &mut meter(),
        )
        .unwrap();
        assert!(
            matches!(replay(callables, &missing, &mut meter()), Err(Error::SignatureLayouts(
            lir::ExactCallableAbiError::MissingValueLayout { exact: actual }
        )) if exact == actual)
        );
    }
    let mut measured = meter();
    replay(callables, layouts, &mut measured).unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    replay(callables, layouts, &mut shared).unwrap();
    assert!(replay(callables, layouts, &mut shared).is_err());
    for limits in [
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(replay(callables, layouts, &mut BudgetMeter::new(limits)).is_err());
    }
}
