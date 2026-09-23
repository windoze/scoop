use super::*;
use lir::LayoutAbiSectionSourceAuthorityV1;

pub(super) fn check(
    input: scoop_lir_lower::LayoutAbiExportInputV1<'_>,
    dependencies: scoop_lir_lower::LayoutAbiExportDependenciesV1<'_>,
) {
    let source =
        scoop_lir_lower::LayoutAbiSourceProjectionV1::from_input(input, dependencies, &mut meter())
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
}
