use super::*;

pub(super) fn check(
    name: &str,
    input: scoop_slib::LirInitializationAbiValidatedCrossConeLayoutClosure<'_>,
    units: &[mir::MirTypeBridgeInitializationUnitV1],
    layout: &lir::CrossConeLayoutAbiSectionV1<'_>,
) {
    let checked = input
        .replay_lir_strong_production()
        .unwrap_or_else(|error| panic!("{name} shared Strong V2 replay: {error}"));
    assert_eq!(checked.current(), ConeIdentity::CORE);
    assert_eq!(checked.dependency_first().count(), 1);
    assert_eq!(checked.dependency_count(ConeIdentity::CORE), Some(0));
    let current = checked.artifact(ConeIdentity::CORE).unwrap();
    assert_eq!(current.lir_exports().layouts(), layout.layouts());
    assert_eq!(
        current.lir_exports().shape_support(),
        layout.shape_support()
    );
    assert_eq!(current.initialization_units(), units);
    let strong = current.lir_strong_production();
    assert!(strong.external_bridges().bridges().is_empty());
    assert!(strong.initialization_cycle_abi().is_some());
    assert!(!strong.digest_finalization_plan().nodes().is_empty());
    if name.starts_with("shared-production-") {
        let dump = format!(
            "callables={}\ntypes={}\nsafepoints={}\nimmortals={}\nstorages={}\ninitializations={}\ndigests={}\n",
            strong.callable_registrations().registrations().len(),
            strong.type_registrations().registrations().len(),
            strong.safepoint_registrations().registrations().len(),
            strong.immortal_registrations().registrations().len(),
            strong.static_storage_registrations().registrations().len(),
            strong.initialization_registrations().registrations().len(),
            strong.digest_finalization_plan().nodes().len(),
        );
        let snapshot = crate::workspace_root()
            .join("tests/fixtures/m23-core-layout-exports")
            .join(format!("{name}.production.snap"));
        if std::env::var_os("SCOOP_UPDATE_CORE_LAYOUT_EXPORTS").is_some() {
            std::fs::write(&snapshot, &dump).unwrap();
        }
        assert_eq!(dump, std::fs::read_to_string(snapshot).unwrap());
    }
    let dependencies = checked
        .replay_mir_dependency_graph()
        .unwrap_or_else(|error| panic!("{name} shared MIR dependency graph: {error}"));
    assert_eq!(dependencies.current(), ConeIdentity::CORE);
    assert_eq!(dependencies.dependency_first().count(), 1);
    let current = dependencies.artifact(ConeIdentity::CORE).unwrap();
    assert_eq!(current.initialization_units(), units);
    assert!(
        current
            .mir_dependency_transport()
            .selected_relations()
            .is_empty()
    );
}
