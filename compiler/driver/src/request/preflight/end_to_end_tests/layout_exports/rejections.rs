use super::*;
use scoop_lir_lower::{
    LayoutAbiExportDependenciesV1, LayoutAbiExportInputV1, LayoutAbiExportLoweringError as Error,
    lower_layout_abi_exports as produce,
};

pub(super) fn check(
    input: LayoutAbiExportInputV1<'_>,
    dependencies: LayoutAbiExportDependenciesV1<'_>,
) {
    assert!(matches!(
        produce(
            input,
            LayoutAbiExportDependenciesV1::default(),
            &mut meter()
        ),
        Err(Error::Layout(
            scoop_lir_lower::ExactLayoutLoweringError::MissingDependency(_)
        )) | Err(Error::MissingLayout(_))
    ));
    let dependency = dependencies.layouts[0];
    assert!(
        matches!(produce(input, LayoutAbiExportDependenciesV1 { layouts: &[dependency, dependency], ..dependencies }, &mut meter()),
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
            dependencies,
            &mut meter()
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
            dependencies,
            &mut meter()
        ),
        Err(Error::Descriptor(lir::ExactDescriptorError::Diagnostic(_)))
    ));
    let mut measured = meter();
    produce(input, dependencies, &mut measured).unwrap();
    let usage = measured.usage();
    assert!(usage.validation_work_units > 0 && usage.owned_bytes > 0);
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: usage.validation_work_units,
        ..DecodeLimits::default()
    });
    produce(input, dependencies, &mut shared).unwrap();
    assert!(produce(input, dependencies, &mut shared).is_err());
    for limits in [
        DecodeLimits {
            owned_bytes: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            semantic_table_entries: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(produce(input, dependencies, &mut BudgetMeter::new(limits)).is_err());
    }
}
