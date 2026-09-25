use super::*;
use lir::LayoutAbiSectionSourceAuthorityV1;

pub(super) fn check(
    input: scoop_lir_lower::LayoutAbiExportInputV1<'_>,
    dependencies: scoop_lir_lower::LayoutAbiExportDependenciesV1<'_>,
    mir_source: &scoop_mir_lower::MirTypeBridgeSourceProjectionV1,
) {
    let source = scoop_lir_lower::LayoutAbiSourceProjectionV1::from_input(
        input,
        dependencies,
        mir_source,
        &mut meter(),
    )
    .unwrap();
    let exports =
        scoop_lir_lower::lower_layout_abi_exports(input, dependencies, &mut meter()).unwrap();
    source
        .validate_local_exports(&exports, &mut meter())
        .unwrap();
    source.validate_physical_imports(&[], &mut meter()).unwrap();
    let roots = source.committed_semantic_roots().unwrap();
    assert!(!roots.is_empty());
    assert!(
        roots
            .iter()
            .all(|root| root.provider() == ConeIdentity::CORE
                && matches!(root.target(), lir::LayoutAbiSemanticTargetV1::Layout(_)))
    );
    for root in roots {
        let lir::LayoutAbiSemanticTargetV1::Layout(layout) = root.target() else {
            unreachable!()
        };
        let source = dependencies
            .layouts
            .iter()
            .find_map(|table| table.get(layout))
            .unwrap();
        assert_eq!(
            source.identity().physical_definition().provider(),
            root.provider()
        );
        assert!(exports.layouts().get(layout).is_none());
    }
    reject_source_drift(input, dependencies, mir_source);
    let mut measured = meter();
    scoop_lir_lower::LayoutAbiSourceProjectionV1::from_input(
        input,
        dependencies,
        mir_source,
        &mut measured,
    )
    .unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    scoop_lir_lower::LayoutAbiSourceProjectionV1::from_input(
        input,
        dependencies,
        mir_source,
        &mut shared,
    )
    .unwrap();
    assert!(
        scoop_lir_lower::LayoutAbiSourceProjectionV1::from_input(
            input,
            dependencies,
            mir_source,
            &mut shared,
        )
        .is_err()
    );
}

fn reject_source_drift(
    input: scoop_lir_lower::LayoutAbiExportInputV1<'_>,
    dependencies: scoop_lir_lower::LayoutAbiExportDependenciesV1<'_>,
    mir_source: &scoop_mir_lower::MirTypeBridgeSourceProjectionV1,
) {
    if input.bridge.types().records().is_empty() {
        return;
    }
    let altered = mir::MirTypeBridgeExportConstituentsV1::new(
        mir::CanonicalParamFreeMirTypeExportsV1::try_new(vec![]).unwrap(),
        input.bridge.callables().clone(),
        input.bridge.dispatch().clone(),
        input.bridge.objects().clone(),
        input.bridge.shapes().clone(),
        input.bridge.initialization_uses().clone(),
    );
    let error = scoop_lir_lower::LayoutAbiSourceProjectionV1::from_input(
        scoop_lir_lower::LayoutAbiExportInputV1 {
            bridge: &altered,
            ..input
        },
        dependencies,
        mir_source,
        &mut meter(),
    )
    .err()
    .unwrap();
    let scoop_lir_lower::LayoutAbiSourceProjectionError::MirSource(error) = error else {
        panic!("expected source drift rejection, found {error}")
    };
    let source = error.downcast_ref::<mir::MirTypeBridgeSourceJoinError<
        scoop_mir_lower::MirTypeBridgeSourceProjectionError,
    >>();
    assert!(matches!(
        source,
        Some(mir::MirTypeBridgeSourceJoinError::Inventory(
            mir::MirTypeBridgeSourceInventoryV1::Types
        ))
    ));
}
