use super::*;

#[test]
fn dependency_method_queries_keep_stable_binders_and_apply_owner_bounds() {
    const PROVIDER: &str = r#"
        public annotation class Mark
        public interface View<T> {}
        public class Box<T> public constructor(private val seed: T) {
            public fun copy(first: T, second: T = first): T = second
            public fun <U: View<T>> accept(value: U): U = value
            context(view: View<T>) public fun read(value: T): T = value
            private fun hidden(): Int = 1
        }
        public interface Reader<T> {
            public fun read(): T
            public fun answer(): Int = 42
            public val item: T
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
            let mut resolve = |text| {
                queries
                    .resolve_type(context, &scoop_parser::parse_type(text).unwrap())
                    .unwrap()
            };
            let box_type = resolve("Box<String>");
            let other = resolve("Box<Int>");
            let view = resolve("View<String>");
            let reader = resolve("Reader<Int>");
            let integer = resolve("Int");
            let methods = queries.nominal_methods(context, box_type).unwrap().unwrap();
            let json =
                scoop_generator_protocol::member_description(queries, function, box_type).unwrap();
            assert_eq!(json["methods"][0]["name"], "copy");
            assert_eq!(
                json["methods"][0]["parameters"][0]["valueType"]["appliedType"],
                serde_json::to_value(scoop_generator_protocol::QueryJson(
                    &queries
                        .type_signature(context.into(), methods[0].parameters[0].value_type.applied)
                        .unwrap()
                ))
                .unwrap()
            );
            assert_eq!(
                json["methods"][1]["result"]["appliedType"],
                serde_json::json!({"kind": "binder", "depth": 0, "index": 0})
            );
            assert_eq!(
                json["methods"][0]["origin"],
                serde_json::to_value(scoop_generator_protocol::QueryJson(&methods[0].origin))
                    .unwrap()
            );
            assert_eq!(json["methods"][3]["access"]["accessible"], false);
            assert_eq!(json["methods"][1]["body"]["availability"], "available");
            for text in ["Unit", "Any", "Ptr<Int>", "FunPtr<(Int) -> Boolean>"] {
                let ty = queries
                    .resolve_type(context, &scoop_parser::parse_type(text).unwrap())
                    .unwrap();
                let core =
                    scoop_generator_protocol::member_description(queries, function, ty).unwrap();
                assert!(!core.is_null(), "imported core declaration: {text}");
            }
            assert_eq!(
                methods
                    .iter()
                    .map(|method| method.name.as_str())
                    .collect::<Vec<_>>(),
                ["copy", "accept", "read", "hidden"]
            );
            let own = &methods[1].type_parameters[0];
            let binders = methods[1].declaration_binders();
            let result = methods[1].result.declaration;
            assert!(queries.describe_type(context.into(), result).is_err());
            let hir::SemanticTypeStructure::Parameter(parameter) = queries
                .describe_type((&binders).into(), result)
                .unwrap()
                .structure
            else {
                panic!("original dependency method binder")
            };
            assert_eq!(parameter.id, own.declaration.id);
            assert_eq!(
                queries.type_signature((&binders).into(), result).unwrap(),
                scoop_identity::SignatureTypeKey::Binder { depth: 0, index: 0 }
            );
            let bound = own.declaration.nominal_bounds_in_source_order()[0].ty();
            let scoop_identity::SignatureTypeKey::NominalApplication { arguments, .. } =
                queries.type_signature((&binders).into(), bound).unwrap()
            else {
                panic!("original dependency owner binder")
            };
            assert_eq!(
                arguments.as_slice(),
                [scoop_identity::SignatureTypeKey::Binder { depth: 1, index: 0 }]
            );
            assert_eq!(own.applied.nominal_bounds_in_source_order()[0].ty(), view);
            assert_ne!(
                own.declaration.nominal_bounds_in_source_order()[0].ty(),
                view
            );
            for ty in [other, box_type] {
                let repeated = queries.nominal_methods(context, ty).unwrap().unwrap();
                assert_eq!(
                    repeated[1].type_parameters[0].declaration.id,
                    own.declaration.id
                );
                assert_eq!(
                    repeated[1].parameters[0].value_type.declaration,
                    methods[1].parameters[0].value_type.declaration
                );
                assert_eq!(repeated[1].identity, methods[1].identity);
            }
            let hir::SemanticParameterCalling::Default(default) = &methods[0].parameters[1].calling
            else {
                panic!("default")
            };
            assert_eq!(
                default.arguments,
                [methods[0].parameters[0].value_type.applied]
            );
            assert!(queries.default_definition(default.definition).is_some());
            assert!(matches!(
                queries.body(methods[1].callable),
                Some(Availability::Available(_))
            ));
            assert_eq!(methods[2].context_parameters[0].value_type.applied, view);
            assert!(!methods[3].access.accessible);
            assert!(methods[0].access.accessible);
            assert_ne!(methods[0].origin.source.cone(), queries.current_cone());
            assert!(
                PROVIDER
                    [methods[1].origin.span.start as usize..methods[1].origin.span.end as usize]
                    .contains("accept")
            );
            let methods = queries.nominal_methods(context, reader).unwrap().unwrap();
            assert_eq!(methods.len(), 2);
            assert_eq!(methods[0].modality, hir::CallableModalityV1::Abstract);
            assert!(matches!(
                queries.body(methods[0].callable),
                Some(Availability::NoBody)
            ));
            assert_eq!(
                methods[1].modality,
                hir::CallableModalityV1::InterfaceDefault
            );
            let methods = queries.nominal_methods(context, integer).unwrap().unwrap();
            assert!(methods.iter().any(|method| method.name == "plus"));
            self.0 = true;
            Ok(Vec::new())
        }
    }
    let mut inspect = Inspect(false);
    support::with_provider(PROVIDER, "@Mark fun inspect() {}", &mut inspect);
    assert!(inspect.0);
}
