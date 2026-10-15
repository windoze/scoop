use super::*;

pub(super) fn check(
    input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
) {
    let first = section.initialization_uses().records()[0];
    assert!(
        mir::SelectedExternalInitializationUseV1::try_new(
            section.provider(),
            input.identities,
            first.local_unit(),
            section.provider(),
            first.dependency_unit(),
            first.cause()
        )
        .is_err()
    );
    assert!(mir::CanonicalMirExternalInitializationUsesV1::try_new(vec![first, first]).is_err());
}
