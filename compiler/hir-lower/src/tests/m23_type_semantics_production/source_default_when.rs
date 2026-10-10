use super::*;

#[test]
fn exhaustive_default_when_retains_the_subject_reference_at_the_when_origin() {
    let source = r#"
        public enum Choice { Left, Right }

        public fun enumDefault(value: Choice, result: Int = when (value) {
            case Left -> 1
            case Right -> 2
        }): Int = result

        public fun booleanDefault(value: Boolean, result: Int = when (value) {
            true -> 1
            false -> 2
        }): Int = result

        public fun wildcardDefault(value: Choice, result: Int = when (value) {
            case _ -> 3
        }): Int = result
    "#;
    source_dispatch::with_hir_source(source, |output, _| {
        let templates =
            hir::CanonicalExportDefaultTemplatesV1::from_dependency_hir(output).unwrap();
        assert_eq!(templates.records().len(), 3);
        for template in templates.records() {
            template
                .validate_reference_closure(&scoop_wire::WirePath::root())
                .unwrap_or_else(|error| panic!("{:?}: {error}", template.key()));
        }
    });
}
