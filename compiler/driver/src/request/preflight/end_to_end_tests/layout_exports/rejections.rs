use super::*;
use scoop_lir_lower::{
    LayoutAbiExportDependenciesV1, LayoutAbiExportInputV1, LayoutAbiExportLoweringError as Error,
    lower_layout_abi_exports as produce,
};

pub(super) fn check(
    input: LayoutAbiExportInputV1<'_>,
    dependencies: LayoutAbiExportDependenciesV1<'_>,
) {
    let dependency = dependencies.layouts[0];
    assert!(
        matches!(produce(input, LayoutAbiExportDependenciesV1 { layouts: &[dependency, dependency], ..dependencies }),
        Err(Error::DuplicateProvider(provider)) if provider == dependency.provider())
    );
    let empty = mir::MirTypeBridgeExportConstituentsV1::new(
        mir::CanonicalParamFreeMirTypeExportsV1::try_new(vec![]).unwrap(),
        input.bridge.callables().clone(),
        input.bridge.dispatch().clone(),
        input.bridge.objects().clone(),
        input.bridge.shapes().clone(),
        input.bridge.initialization_uses().clone(),
    );
    assert!(matches!(
        produce(
            LayoutAbiExportInputV1 {
                bridge: &empty,
                ..input
            },
            dependencies
        ),
        Err(Error::Layout(
            scoop_lir_lower::ExactLayoutLoweringError::MissingMirShape(_)
        ))
    ));
    assert!(matches!(
        produce(
            LayoutAbiExportInputV1 {
                coordinates: &[],
                ..input
            },
            dependencies
        ),
        Err(Error::Descriptor(lir::ExactDescriptorError::Diagnostic(_)))
    ));

    produce(input, dependencies).unwrap();
}
