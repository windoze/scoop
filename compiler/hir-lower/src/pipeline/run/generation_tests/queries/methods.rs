use super::types::resolve;
use super::*;

#[test]
fn method_queries_preserve_definition_binders_defaults_effects_and_access() {
    let source = r#"
        annotation class Mark
        interface View<T> {}
        class Box<T>(private val seed: T) {
            fun copy(first: T, second: T = first): T = second
            fun <U: View<T>> accept(value: U): U = value
            context(view: View<T>) fun read(value: T): T = value
            private fun hidden(): Int = 1
        }
        interface Reader<T> {
            fun read(): T
            fun answer(): Int = 42
            val item: T
        }
        @Mark fun inspect() {}
    "#;
    let mut seen = Vec::new();
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let context = SemanticContext::Function(marked_functions(queries)[0]);
        let string = resolve(queries, context, "String");
        let box_type = resolve(queries, context, "Box<String>");
        let view = resolve(queries, context, "View<String>");
        let methods = queries.nominal_methods(context, box_type).unwrap().unwrap();
        assert_eq!(
            methods
                .iter()
                .map(|method| method.name.as_str())
                .collect::<Vec<_>>(),
            ["copy", "accept", "read", "hidden"]
        );
        let copy = &methods[0];
        assert_eq!(copy.parameters[0].value_type.applied, string);
        assert_ne!(copy.parameters[0].value_type.declaration, string);
        assert_eq!(copy.result, copy.parameters[1].value_type);
        let hir::SemanticParameterCalling::Default(default) = &copy.parameters[1].calling else {
            panic!("declared default")
        };
        assert_eq!(default.arguments, [string]);
        assert!(queries.default_definition(default.definition).is_some());
        let accept = &methods[1];
        let parameter = &accept.type_parameters[0];
        assert_eq!(parameter.declaration.id, parameter.applied.id);
        assert_eq!(
            parameter.applied.nominal_bounds_in_source_order()[0].ty(),
            view
        );
        assert_ne!(
            parameter.declaration.nominal_bounds_in_source_order()[0].ty(),
            view
        );
        assert_eq!(
            accept.parameters[0].value_type.declaration,
            accept.parameters[0].value_type.applied
        );
        assert_eq!(methods[2].context_parameters[0].value_type.applied, view);
        assert_eq!(
            methods[2].context_parameters[0].label,
            hir::ContextParameterLabel::Named("view".into())
        );
        assert!(!methods[3].access.accessible);
        assert_eq!(
            methods[3].access.visibility,
            hir::DeclaredVisibilityV1::Private
        );
        for method in methods {
            assert_eq!(method.receiver.applied, box_type);
            assert!(matches!(
                queries.body(method.callable),
                Some(Availability::Available(_))
            ));
            let SemanticCallable::Current(function) = method.callable else {
                panic!("current")
            };
            seen.push((function, method.identity));
        }
        let reader = resolve(queries, context, "Reader<Int>");
        let methods = queries.nominal_methods(context, reader).unwrap().unwrap();
        assert_eq!(
            methods.len(),
            2,
            "property accessors are not ordinary methods"
        );
        assert_eq!(methods[0].modality, hir::CallableModalityV1::Abstract);
        assert!(matches!(
            queries.body(methods[0].callable),
            Some(Availability::NoBody)
        ));
        assert_eq!(
            methods[1].modality,
            hir::CallableModalityV1::InterfaceDefault
        );
        assert!(matches!(
            queries.body(methods[1].callable),
            Some(Availability::Available(_))
        ));
        Ok(Vec::new())
    });
    let module = compile(source, &mut driver).unwrap();
    for (function, identity) in seen {
        let hir::HirFunctionIdentity::Source(source) = &module.function_identities[function] else {
            panic!("source identity")
        };
        let actual = match source {
            hir::HirSourceFunctionIdentity::Plain(record) => {
                scoop_identity::CallableTemplateOrigin::Function(record.id())
            }
            hir::HirSourceFunctionIdentity::Generic(record) => {
                scoop_identity::CallableTemplateOrigin::GenericFunction(record.id())
            }
        };
        assert_eq!(identity, actual);
    }
}
