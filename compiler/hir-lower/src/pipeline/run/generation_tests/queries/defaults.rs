use super::types::resolve;
use super::*;

#[test]
fn default_application_keeps_method_binders_separate_from_applied_owner_types() {
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let context = SemanticContext::Function(marked_functions(queries)[0]);
        let owner = resolve(queries, context, "Box<String>");
        let string = resolve(queries, context, "String");
        let methods = queries.nominal_methods(context, owner).unwrap().unwrap();
        let method = &methods[0];
        let hir::SemanticParameterCalling::Default(default) = &method.parameters[1].calling else {
            panic!("method default")
        };
        let binders = method.declaration_binders();
        let application = queries
            .default_application((&binders).into(), default.definition, &default.arguments)
            .unwrap()
            .unwrap();
        let definition = application.definition();
        let binder = scoop_identity::SignatureTypeKey::Binder { depth: 0, index: 0 };
        assert_eq!(application.provider().nominal_owner_binder_arity(), 1);
        assert_eq!(application.provider().callable_own_binder_arity(), 1);
        assert_eq!(
            application.applied_type(definition.result()).unwrap(),
            binder
        );
        assert_eq!(definition.result(), &binder);
        let expected_owner = queries.type_signature(context.into(), string).unwrap();
        assert_eq!(
            application.arguments().arguments(),
            &[expected_owner, binder]
        );
        Ok(Vec::new())
    });
    compile(
        "annotation class Mark\nclass Box<T>(val seed: T) { fun <U> choose(first: U, second: U = first): U = second }\n@Mark fun inspect() {}",
        &mut driver,
    ).unwrap();
}

#[test]
fn inherited_source_default_uses_the_definition_root_and_keeps_its_tree() {
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let context = SemanticContext::Function(marked_functions(queries)[0]);
        let owner = resolve(queries, context, "Choice<Int>");
        let int = resolve(queries, context, "Int");
        let methods = queries.nominal_methods(context, owner).unwrap().unwrap();
        let method = &methods[0];
        let hir::SemanticParameterCalling::Default(default) = &method.parameters[1].calling else {
            panic!("inherited source default")
        };
        let application = queries
            .default_application(context.into(), default.definition, &default.arguments)
            .unwrap()
            .unwrap();
        let definition = application.definition();
        assert_ne!(
            definition.key().owner(),
            definition.definition_root().declaration()
        );
        assert_eq!(application.provider().nominal_owner_binder_arity(), 2);
        assert_eq!(application.arguments().len_u32(), 2);
        assert_eq!(
            application.applied_type(definition.result()).unwrap(),
            queries.type_signature(context.into(), int).unwrap(),
        );
        assert_eq!(
            definition.result(),
            &scoop_identity::SignatureTypeKey::Binder { depth: 0, index: 1 },
        );
        let original =
            serde_json::to_value(scoop_generator_protocol::BodyJson::new(definition.as_ref()))
                .unwrap();
        let applied =
            serde_json::to_value(scoop_generator_protocol::applied_default(&application)).unwrap();
        assert_eq!(
            original["result"],
            serde_json::json!({"kind": "binder", "depth": 0, "index": 1})
        );
        assert_eq!(
            applied["result"],
            serde_json::json!(scoop_generator_protocol::QueryJson(
                &queries.type_signature(context.into(), int).unwrap()
            ))
        );
        assert_ne!(applied["key"]["owner"], applied["definitionRoot"]);
        assert_eq!(applied["key"], original["key"]);
        assert_eq!(
            applied["previousParameters"],
            original["previousParameters"]
        );
        assert_eq!(applied["origin"], original["origin"]);
        Ok(Vec::new())
    });
    compile(
        "annotation class Mark\ninterface Picker<A, B> { fun choose(first: B, second: B = first): B }\nclass Choice<T> : Picker<String, T> { override fun choose(first: T, second: T): T = second }\n@Mark fun inspect() {}",
        &mut driver,
    ).unwrap();
}
