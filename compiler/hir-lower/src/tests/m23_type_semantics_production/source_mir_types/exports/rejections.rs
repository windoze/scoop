use super::*;

pub(super) fn check(
    input: MirTypeBridgeExportInputV1<'_>,
    dependencies: &mir::CanonicalParamFreeMirTypeExportsV1,
) {
    assert!(produce(input, &[]).is_err());
    assert!(matches!(
        produce(input, &[dependencies, dependencies]),
        Err(Error::Lookup(
            mir::MirTypeBridgeLookupError::DuplicateType { .. }
        ))
    ));
    let wrong = mir::CrossConeMirBridgeSectionV1::try_new(
        ConeIdentity::CORE,
        input.mir.foundation(),
        vec![],
        vec![],
    )
    .unwrap();
    assert!(matches!(
        produce(
            MirTypeBridgeExportInputV1 {
                ordinary: &wrong,
                ..input
            },
            &[dependencies]
        ),
        Err(Error::ProviderMismatch)
    ));

    produce(input, &[dependencies]).unwrap();
}
