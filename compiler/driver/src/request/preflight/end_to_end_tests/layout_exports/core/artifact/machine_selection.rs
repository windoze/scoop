use scoop_lir::*;

pub(super) use super::super::super::support::physical::Source;

pub(super) fn empty_exports(
    foundation: &OdrFreeLirFoundation,
    target: LirTargetProfile,
) -> LayoutAbiExportConstituentsV1 {
    let layouts = CanonicalExactLayoutExportsV1::try_new(target, foundation, Vec::new()).unwrap();
    let descriptors =
        CanonicalExactDescriptorExportsV1::try_new(target, foundation, Vec::new()).unwrap();
    let dispatch =
        CanonicalExactDispatchExportsV1::try_new(target, foundation, Vec::new()).unwrap();
    let callables =
        CanonicalExactCallableAbiExportsV1::try_new(target, foundation, Vec::new()).unwrap();
    let shapes = CanonicalParamFreeShapeSupportExportsV1::from_sources(
        &[],
        &layouts,
        &descriptors,
        foundation,
    )
    .unwrap();
    LayoutAbiExportConstituentsV1::try_new(layouts, descriptors, dispatch, callables, shapes)
        .unwrap()
}
