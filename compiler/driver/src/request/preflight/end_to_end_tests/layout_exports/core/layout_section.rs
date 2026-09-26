use super::*;

pub(super) fn check(
    mir_input: scoop_mir_lower::MirTypeBridgeExportInputV1<'_>,
    input: scoop_lir_lower::LayoutAbiExportInputV1<'_>,
    exports: lir::LayoutAbiExportConstituentsV1,
) -> lir::CrossConeLayoutAbiSectionV1<'static> {
    let mir_source = scoop_mir_lower::MirTypeBridgeSourceProjectionV1::from_input(
        mir_input,
        scoop_mir_lower::MirTypeBridgeDependencyTablesV1 {
            types: &[],
            callables: &[],
            dispatch: &[],
        },
    )
    .unwrap();
    let roots = scoop_lir_lower::lower_layout_abi_dependencies(
        input,
        scoop_lir_lower::LayoutAbiExportDependenciesV1::default(),
        &exports,
        mir::MirTypeBridgeSectionSourceAuthorityV1::committed_external_uses(&mir_source).unwrap(),
        &[],
    )
    .unwrap();
    let section = lir::CrossConeLayoutAbiSectionV1::try_new(exports, &[], vec![], &roots).unwrap();
    assert!(section.selected().is_empty());
    assert!(section.selected().physical_imports().records().is_empty());
    section
}

pub(super) fn snapshot(
    name: &str,
    fixtures: &Path,
    section: &lir::CrossConeLayoutAbiSectionV1<'_>,
    objects: &scoop_codegen::EmittedStrongObjectSetV2,
) {
    assert!(!objects.members().is_empty());
    let production = objects.production();
    let dump = format!(
        "layouts={} descriptors={} dispatch={} callables={} shapes={} selected={} physical={}\nregistrations: types={} callables={} initialization={}\nobject members={}\n",
        section.layouts().records().len(),
        section.descriptors().records().len(),
        section.dispatch().records().len(),
        section.callables().records().len(),
        section.shape_support().records().len(),
        section.selected().len(),
        section.selected().physical_imports().records().len(),
        production.type_registrations().registrations().len(),
        production.callable_registrations().registrations().len(),
        production
            .initialization_registrations()
            .registrations()
            .len(),
        objects.members().len(),
    );
    let snapshot = fixtures.join(format!("{name}.lir-section.snap"));
    if std::env::var_os("SCOOP_UPDATE_CORE_LAYOUT_EXPORTS").is_some() {
        std::fs::write(&snapshot, &dump).unwrap();
    }
    assert_eq!(dump, std::fs::read_to_string(snapshot).unwrap());
}
