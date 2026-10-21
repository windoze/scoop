use super::*;

#[test]
fn dependency_property_queries_keep_private_setters_definition_types_and_published_bodies() {
    const PROVIDER: &str = r#"
        public annotation class Mark
        public annotation class Label(val text: String)
        public class Box<T> public constructor(private val value: T) {
            @get:Label("read") @set:Label("write")
            public var mirror: T get() = value
                private set(next) {}
        }
        public interface View<T> { public val item: T }
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
            let parameter_type = |index: usize| {
                let hir::ExportParameterCalling::Required { value_type } =
                    signature.parameters[index].calling
                else {
                    panic!("required parameter")
                };
                value_type
            };
            let box_type = parameter_type(0);
            let view_type = parameter_type(1);
            let properties = queries
                .nominal_properties(context, box_type)
                .unwrap()
                .unwrap();
            assert_eq!(
                properties
                    .iter()
                    .map(|property| property.name.as_str())
                    .collect::<Vec<_>>(),
                ["value", "mirror"]
            );
            assert!(!properties[0].getter.access.accessible);
            let mirror = &properties[1];
            assert!(mirror.getter.access.accessible);
            assert!(!mirror.setter.as_ref().unwrap().access.accessible);
            assert_eq!(
                mirror.setter.as_ref().unwrap().access.visibility,
                hir::DeclaredVisibilityV1::Private
            );
            assert_ne!(mirror.value_type.declaration, mirror.value_type.applied);
            let fields = queries.nominal_fields(context, box_type).unwrap().unwrap();
            assert_eq!(mirror.value_type, fields.fields[0].value_type);
            assert_ne!(mirror.origin.source.cone(), queries.current_cone());
            assert!(
                PROVIDER[mirror.origin.span.start as usize..mirror.origin.span.end as usize]
                    .contains("var mirror: T")
            );
            for accessor in [&mirror.getter, mirror.setter.as_ref().unwrap()] {
                assert!(matches!(
                    queries.body(accessor.callable),
                    Some(Availability::Available(hir::SemanticBody::Imported(_)))
                ));
                assert!(
                    matches!(queries.describe_annotations(accessor.annotations), hir::SemanticAnnotations::Imported(annotations) if annotations.len() == 1)
                );
            }
            let properties = queries
                .nominal_properties(context, view_type)
                .unwrap()
                .unwrap();
            assert!(properties[0].setter.is_none());
            assert!(matches!(
                queries.body(properties[0].getter.callable),
                Some(Availability::NoBody)
            ));
            self.0 = true;
            Ok(Vec::new())
        }
    }
    let mut inspect = Inspect(false);
    support::with_provider(
        PROVIDER,
        "@Mark fun inspect(box: Box<String>, view: View<Int>) {}",
        &mut inspect,
    );
    assert!(inspect.0);
}
