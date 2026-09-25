use super::*;

pub(super) fn check(
    callables: &mir::StrongCallableBridgeSurfaceV1,
    layouts: &lir::CanonicalExactLayoutExportsV1,
    foundation: &lir::OdrFreeLirFoundation,
    replay: &impl Fn(
        &mir::StrongCallableBridgeSurfaceV1,
        &lir::CanonicalExactLayoutExportsV1,
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
    assert!(replay(&absent, layouts).unwrap().is_none());
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
        )
        .unwrap();
        assert!(
            matches!(replay(callables, &missing), Err(Error::SignatureLayouts(
            lir::ExactCallableAbiError::MissingValueLayout { exact: actual }
        )) if exact == actual)
        );
    }

    replay(callables, layouts).unwrap();
}
