use super::*;

pub(super) fn check(
    input: scoop_lir_lower::LayoutAbiExportInputV1<'_>,
    dependencies: scoop_lir_lower::LayoutAbiExportDependenciesV1<'_>,
    source: &scoop_mir_lower::MirTypeBridgeSourceProjectionV1,
    selected: &lir::StrongProductionDependencySelectionV2<'_>,
    uses: &[lir::StrongExternalInitializationUseV2],
) {
    let first = uses[0];
    let omitted = uses
        .iter()
        .copied()
        .filter(|usage| *usage != first)
        .collect::<Vec<_>>();
    let mut candidates = vec![vec![], omitted];
    let registrations = input
        .registration
        .registration_production()
        .initialization_units();
    if let Some(extra_unit) = registrations
        .registrations()
        .iter()
        .map(|unit| unit.semantic().unit())
        .find(|unit| uses.iter().all(|usage| usage.local_unit() != *unit))
    {
        let mut extra = uses.to_vec();
        extra.push(
            lir::StrongExternalInitializationUseV2::try_new(
                extra_unit,
                first.dependency(),
                selected,
            )
            .unwrap(),
        );
        candidates.push(extra);
    }
    for candidate in candidates {
        let registration = input
            .lir
            .build_production_section_v2(
                input.coordinates[1].clone(),
                &[first.provider()],
                lir::EntryProductionSourceV1::Library,
                selected,
                &candidate,
            )
            .unwrap();
        let error = scoop_lir_lower::LayoutAbiSourceProjectionV1::from_input(
            scoop_lir_lower::LayoutAbiExportInputV1 {
                registration: &registration,
                ..input
            },
            dependencies,
            source,
        )
        .err()
        .unwrap();
        assert!(matches!(
            error,
            scoop_lir_lower::LayoutAbiSourceProjectionError::InitializationEdges(error)
                if matches!(*error, mir::MirObjectBridgeError::InitializationDependencyInventory)
        ));
        let error = scoop_slib::replay_shared_lir_initialization_dependencies(
            input.bridge.initialization_uses(),
            registration
                .registration_production()
                .initialization_units(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            scoop_slib::SharedLirDependencyGraphError::InitializationEdges(error)
                if matches!(*error, mir::MirObjectBridgeError::InitializationDependencyInventory)
        ));
    }
}
