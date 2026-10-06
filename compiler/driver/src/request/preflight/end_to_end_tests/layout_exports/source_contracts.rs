use super::*;

pub(super) fn check(
    input: scoop_lir_lower::LayoutAbiExportInputV1<'_>,
    dependencies: scoop_lir_lower::LayoutAbiExportDependenciesV1<'_>,
    mir_source: &[mir::MirTypeBridgeDependencyV1],
    physical: &[lir::ExternalShapeLinkImportV1],
) {
    let exports = scoop_lir_lower::lower_layout_abi_exports(input, dependencies).unwrap();
    let roots = scoop_lir_lower::lower_layout_abi_dependencies(
        input,
        dependencies,
        &exports,
        mir_source,
        physical,
    )
    .unwrap();
    assert!(!roots.is_empty());
    for root in &roots {
        match root.target() {
            lir::LayoutAbiSemanticTargetV1::Layout(layout) => {
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
            lir::LayoutAbiSemanticTargetV1::Descriptor(exact) => {
                assert!(physical.iter().any(|import| {
                    import.provider() == root.provider()
                        && import.subject()
                            == lir::ExternalStrongShapeSubjectV1::TypeDescriptor(exact)
                }));
            }
            lir::LayoutAbiSemanticTargetV1::Callable(callable) => {
                assert!(physical.iter().any(|import| {
                    import.provider() == root.provider()
                        && import.subject() == lir::ExternalStrongShapeSubjectV1::Callable(callable)
                }));
            }
            target => panic!("unexpected source dependency {target:?}"),
        }
    }
}
