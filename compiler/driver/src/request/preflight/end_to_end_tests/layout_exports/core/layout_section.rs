use super::*;

pub(super) fn check(
    mir_input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    input: scoop_lir_lower::LayoutAbiExportInputV1<'_>,
    exports: lir::LayoutAbiExportConstituentsV1,
) -> lir::CrossConeLayoutAbiSectionV1<'static> {
    let mir_source = scoop_mir_lower::lower_type_bridge_dependencies(mir_input).unwrap();
    let roots = scoop_lir_lower::lower_layout_abi_dependencies(
        input,
        scoop_lir_lower::LayoutAbiExportDependenciesV1::default(),
        &exports,
        &mir_source,
        &[],
    )
    .unwrap();
    let section = lir::CrossConeLayoutAbiSectionV1::try_new(exports, &[], vec![], &roots).unwrap();
    assert!(section.selected().is_empty());
    assert!(section.selected().physical_imports().records().is_empty());
    section
}
