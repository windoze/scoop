use super::*;

#[test]
fn constructor_materialization_keeps_derived_equality_source_context() {
    source_dispatch::with_hir_sources(
        &[
            (
                "src/comparison.scoop",
                r#"
                public enum Record<T> { Empty, Text(String) }
                public class Witness<T> public constructor(left: String?, right: String?, record: Record<Int>, other: Record<Int>) {
                    public val equal: Boolean = left == right
                    public val sameRecord: Boolean = record == other
                }
            "#,
            ),
            (
                "src/holder.scoop",
                r#"
                public class Holder public constructor(value: String?, other: String?, record: Record<Int>, otherRecord: Record<Int>) {
                    public val equal: Boolean = value == other
                    public val sameRecord: Boolean = record == otherRecord
                }
            "#,
            ),
        ],
        |output, _| {
            let local = output.output().local.module();
            let export = output.output().export.module();
            assert!(local.functions.iter().any(|(_, function)| matches!(
                function.materialization.template(),
                scoop_identity::CallableTemplateOwner::Generated(_)
            )));
            local
                .visit_executable_expressions(|occurrence| {
                    let evaluation = occurrence.expression.origin.evaluation;
                    assert_eq!(
                        export.source_contexts[evaluation.context].source(),
                        &export.source_files[evaluation.file as usize].identity,
                        "{:?}",
                        occurrence.position
                    );
                    Ok::<_, std::convert::Infallible>(())
                })
                .unwrap();
            public_interface(output);
        },
    );
}

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
