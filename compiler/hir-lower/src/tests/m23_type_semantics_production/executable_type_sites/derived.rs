use super::*;

#[test]
fn repeated_open_equality_keeps_each_comparison_context() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/regressions/equality/derived-equality-origins/provider/src/main.scoop"
    ));
    with_hir_source(source, |output, _| {
        let interface = public_interface(output);
        let mut foundation = hir::CanonicalHirFoundation::from_dependency_output(output).unwrap();
        foundation
            .complete_cross_cone_interface_source_points(
                output.output().export.module(),
                &interface,
            )
            .unwrap();
        let cone = output.output().export.cone;
        let mut sites = 0;
        for reference in interface.external_references().records() {
            let calls = reference
                .call_sites()
                .records()
                .iter()
                .map(|site| (site.position(), site.origin()));
            let types = reference.type_sites().records().iter().filter_map(|site| {
                site.as_expression()
                    .map(|site| (site.position(), site.origin()))
            });
            for (position, origin) in calls.chain(types) {
                foundation
                    .validate_executable_evaluation_origin(
                        cone,
                        position.root,
                        origin.evaluation(),
                        &foundation,
                    )
                    .unwrap_or_else(|error| panic!("{position:?}: {error}"));
                foundation
                    .validate_definition_origin_location(cone, origin.definition())
                    .unwrap();
                sites += 1;
            }
        }
        assert!(sites > 0);
    });
}
