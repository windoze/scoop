use super::*;

pub(super) fn check(
    name: &str,
    input: scoop_slib::LirExportsValidatedCrossConeLayoutClosure<'_>,
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
    assert!(!strong.digest_finalization_plan().nodes().is_empty());
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
    let dependencies = dependencies
        .replay_lir_dependency_graph()
        .unwrap_or_else(|error| panic!("{name} shared LIR dependency graph: {error}"));
    assert_eq!(dependencies.current(), ConeIdentity::CORE);
    assert_eq!(dependencies.dependency_first().count(), 1);
    let current = dependencies.artifact(ConeIdentity::CORE).unwrap();
    assert_eq!(current.lir_exports().layouts(), layout.layouts());
    assert_eq!(current.initialization_units(), units);
    assert!(
        current
            .lir_dependency_transport()
            .selected_relations()
            .is_empty()
    );
    dependencies
        .replay_physical_imports()
        .map(|physical| {
            assert_eq!(physical.current(), ConeIdentity::CORE);
            assert_eq!(physical.dependency_first().count(), 1);
            let current = physical.artifact(ConeIdentity::CORE).unwrap();
            assert_eq!(current.lir_exports(), layout.exports());
            assert_eq!(current.initialization_units(), units);
            assert!(current.lir_physical_imports().records().is_empty());
        })
        .unwrap_or_else(|error| panic!("{name} shared physical contract and Strong join: {error}"));
}
