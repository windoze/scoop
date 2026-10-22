use super::*;

#[test]
fn dependency_constructor_queries_keep_source_parameters_and_default_binder_arguments() {
    const PROVIDER: &str = r#"
        public annotation class Mark
        public struct Pair<T>(@param:Mark val first: T, val second: T = first)
        public class Row<T> public constructor(seed: T, public val value: T = seed)
        public class Many<T> public constructor(vararg values: T)
        public class Fallback public constructor(vararg values: Int = [1, 2])
        public enum Choice<T> { Empty, Named(val first: T, val second: T = first) }
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
            let parameter_type = |index: usize| match signature.parameters[index].calling {
                hir::ExportParameterCalling::Required { value_type } => value_type,
                _ => panic!("required input"),
            };
            let pair = parameter_type(0);
            let constructors = queries
                .nominal_constructors(context, pair)
                .unwrap()
                .unwrap();
            assert_eq!(constructors.len(), 1);
            let constructor = &constructors[0];
            assert_eq!(constructor.kind, hir::SemanticConstructorKind::Primary);
            let fields = queries
                .nominal_fields(context, pair)
                .unwrap()
                .unwrap()
                .fields;
            for (parameter, field) in constructor.parameters.iter().zip(&fields) {
                assert_eq!(
                    parameter.target,
                    hir::StaticParameterTarget::Field(field.identity)
                );
                assert_eq!(parameter.value_type, field.value_type);
            }
            assert!(
                matches!(queries.describe_annotations(constructor.parameters[0].annotations), hir::SemanticAnnotations::Imported(annotations) if annotations.len() == 1)
            );
            let hir::SemanticParameterCalling::Default(default) =
                &constructor.parameters[1].calling
            else {
                panic!("imported default")
            };
            assert_eq!(default.arguments, [fields[0].value_type.applied]);
            let Some(hir::SemanticDefaultDefinition::Imported(template)) =
                queries.default_definition(default.definition)
            else {
                panic!("original dependency definition")
            };
            assert_eq!(template.key().parameter_position(), 1);
            assert_ne!(
                template.definition_origin().origin().source().cone(),
                queries.current_cone()
            );
            let projected = queries
                .projected_default(default.definition)
                .unwrap()
                .unwrap();
            assert_eq!(projected.key().owner(), constructor.identity);
            let repeated = queries
                .projected_default(default.definition)
                .unwrap()
                .unwrap();
            assert!(std::sync::Arc::ptr_eq(&projected, &repeated));
            let application = queries
                .default_application(context.into(), default.definition, &default.arguments)
                .unwrap()
                .unwrap();
            assert!(std::sync::Arc::ptr_eq(application.definition(), &projected));
            let expected = queries
                .type_signature(context.into(), fields[0].value_type.applied)
                .unwrap();
            assert_eq!(
                application.applied_type(projected.result()).unwrap(),
                expected
            );
            assert_eq!(
                application
                    .applied_type(projected.body().value().result_type())
                    .unwrap(),
                expected
            );
            let row = parameter_type(1);
            let constructors = queries.nominal_constructors(context, row).unwrap().unwrap();
            assert_eq!(
                constructors[0].parameters[0].target,
                hir::StaticParameterTarget::Plain
            );
            let property = &queries.nominal_properties(context, row).unwrap().unwrap()[0];
            assert_eq!(
                constructors[0].parameters[1].target,
                hir::StaticParameterTarget::Property(property.identity)
            );
            let many = queries
                .nominal_constructors(context, parameter_type(2))
                .unwrap()
                .unwrap();
            assert!(matches!(
                many[0].parameters[0].calling,
                hir::SemanticParameterCalling::Vararg {
                    omission: hir::SemanticVarargOmission::EmptyArray,
                    ..
                }
            ));
            let fallback = queries
                .nominal_constructors(context, parameter_type(3))
                .unwrap()
                .unwrap();
            let hir::SemanticParameterCalling::Vararg {
                omission: hir::SemanticVarargOmission::Default(default),
                ..
            } = &fallback[0].parameters[0].calling
            else {
                panic!("vararg default")
            };
            assert!(queries.default_definition(default.definition).is_some());
            let choice = parameter_type(4);
            let constructors = queries
                .nominal_constructors(context, choice)
                .unwrap()
                .unwrap();
            let variants = queries
                .nominal_fields(context, choice)
                .unwrap()
                .unwrap()
                .variants;
            assert_eq!(constructors.len(), variants.len());
            assert_eq!(
                constructors[1].parameters[0].target,
                hir::StaticParameterTarget::Field(variants[1].fields[0].identity)
            );
            self.0 = true;
            Ok(Vec::new())
        }
    }
    let mut inspect = Inspect(false);
    support::with_provider(
        PROVIDER,
        "@Mark fun inspect(pair: Pair<String>, row: Row<Int>, many: Many<String>, fallback: Fallback, choice: Choice<String>) {}",
        &mut inspect,
    );
    assert!(inspect.0);
}
