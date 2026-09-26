use super::*;

pub(super) fn check(
    input: scoop_lir_lower::LayoutAbiExportInputV1<'_>,
    dependencies: scoop_lir_lower::LayoutAbiExportDependenciesV1<'_>,
    mir_source: &scoop_mir_lower::MirTypeBridgeSourceProjectionV1,
) {
    let exports = scoop_lir_lower::lower_layout_abi_exports(input, dependencies).unwrap();
    let roots = scoop_lir_lower::lower_layout_abi_dependencies(
        input,
        dependencies,
        &exports,
        mir::MirTypeBridgeSectionSourceAuthorityV1::committed_external_uses(mir_source).unwrap(),
        &[],
    )
    .unwrap();
    assert!(!roots.is_empty());
    assert!(
        roots
            .iter()
            .all(|root| root.provider() == ConeIdentity::CORE
                && matches!(root.target(), lir::LayoutAbiSemanticTargetV1::Layout(_)))
    );
    for root in &roots {
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
}
