use super::*;

#[test]
fn dependency_storage_queries_retain_actual_origins_and_generic_substitution() {
    const PROVIDER: &str = r#"
        public annotation class Label(val text: String)
        public struct Box<T>(@Label("值") val value: T, val fixed: Int)
        public enum Choice<T> { Empty, Named(@Label("payload") val value: T), Pair(T, Int) }
        public class Row<T> public constructor(private val value: T) {
            public val computed: Int get() = 1
        }
    "#;
    struct Inspect(bool);
    impl hir::MethodBodyGenerator for Inspect {
        fn generate(
            &mut self,
            queries: &mut dyn SemanticQueries,
        ) -> Result<Vec<hir::GeneratedMethodBody>, Vec<scoop_ast::Diagnostic>> {
            let function = queries
                .annotations()
                .targets
                .iter()
                .find_map(|target| match target.target {
                    hir::SourceAnnotationTarget::Callable(
                        hir::SourceAnnotationCallable::Function(id),
                    ) => Some(id),
                    _ => None,
                })
                .unwrap();
            let context = hir::SemanticContext::Function(function);
            let signature = queries.signature(function);
            let parameters = signature
                .parameters
                .iter()
                .map(|parameter| {
                    let hir::ExportParameterCalling::Required { value_type } = parameter.calling
                    else {
                        panic!("required parameter")
                    };
                    value_type
                })
                .collect::<Vec<_>>();
            let shape = queries
                .nominal_fields(context, parameters[0])
                .unwrap()
                .unwrap();
            let field = &shape.fields[0];
            let hir::SemanticTypeStructure::Nominal(nominal) = queries
                .describe_type(context, parameters[0])
                .unwrap()
                .structure
            else {
                panic!("Box")
            };
            assert_eq!(field.value_type.applied, nominal.arguments[0]);
            assert_ne!(field.value_type.declaration, field.value_type.applied);
            assert_ne!(field.origin.source.cone(), queries.current_cone());
            assert!(
                PROVIDER[field.origin.span.start as usize..field.origin.span.end as usize]
                    .contains("val value: T")
            );
            let hir::SemanticAnnotations::Imported(annotations) =
                queries.describe_annotations(field.annotations)
            else {
                panic!("dependency field annotations")
            };
            assert_eq!(annotations.len(), 1);
            let choice = queries
                .nominal_fields(context, parameters[1])
                .unwrap()
                .unwrap();
            assert_eq!(
                choice
                    .variants
                    .iter()
                    .map(|variant| variant.name.as_str())
                    .collect::<Vec<_>>(),
                ["Empty", "Named", "Pair"]
            );
            assert!(
                PROVIDER[choice.variants[1].origin.span.start as usize
                    ..choice.variants[1].origin.span.end as usize]
                    .starts_with("Named(")
            );
            let field = &choice.variants[1].fields[0];
            assert!(
                PROVIDER[field.origin.span.start as usize..field.origin.span.end as usize]
                    .contains("val value: T")
            );
            assert!(
                matches!(queries.describe_annotations(field.annotations), hir::SemanticAnnotations::Imported(annotations) if annotations.len() == 1)
            );
            let row = queries
                .nominal_fields(context, parameters[2])
                .unwrap()
                .unwrap();
            assert_eq!(row.fields.len(), 1);
            assert_eq!(row.fields[0].name, "value");
            assert!(matches!(
                row.fields[0].storage,
                hir::NominalFieldStorage::PropertyBacking(_)
            ));
            self.0 = true;
            Ok(Vec::new())
        }
    }
    let mut inspect = Inspect(false);
    support::with_provider(
        PROVIDER,
        "@Label(\"inspect\") fun inspect(box: Box<String>, choice: Choice<Int>, row: Row<Int>) {}",
        &mut inspect,
    );
    assert!(inspect.0);
}
