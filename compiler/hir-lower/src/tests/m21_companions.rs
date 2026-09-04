use super::*;

fn public_visibility() -> ast::VisibilitySyntax {
    ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Public,
        span: sp(),
    }
}

fn companion_method() -> ast::FunctionDecl {
    let mut method = method_full(
        false,
        false,
        "answer",
        Vec::new(),
        Some(ty_named("Int")),
        ast::FunctionBody::Expr(Box::new(int_lit(42))),
    );
    method.visibility = public_visibility();
    method
}

fn host_class() -> ast::ClassDecl {
    let Decl::Class(mut host) = class_decl(
        ast::ClassModifier::Final,
        "Host",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    ) else {
        unreachable!("class builder returns a class")
    };
    host.visibility = public_visibility();
    host.type_params = vec![type_param("T")];
    host.members.push(ast::ClassMember::Companion(Box::new(
        ast::CompanionObjectDecl {
            annotations: Vec::new(),
            visibility: public_visibility(),
            name: ast::CompanionNameSyntax::Named(ident("Factory")),
            supertypes: Vec::new(),
            members: vec![ast::ClassMember::Function(companion_method())],
            span: sp(),
        },
    )));
    host
}

fn companion_receiver(function: &hir::Function) -> hir::SingletonValueId {
    let hir::FunctionKind::User(body) = &function.kind else {
        panic!("test function must have a user body")
    };
    let expression = return_value(&body.statements);
    let hir::ExprKind::MethodCall { receiver, .. } = &expression.kind else {
        panic!("companion access must lower to a method call")
    };
    match receiver.kind {
        hir::ExprKind::SingletonValue(value) => value,
        hir::ExprKind::Local(local) => body
            .statements
            .iter()
            .find_map(|statement| {
                let hir::StatementKind::ValDecl { pattern, init } = &statement.kind else {
                    return None;
                };
                let hir::Pattern::Binding { local: candidate } = pattern else {
                    return None;
                };
                if *candidate != local {
                    return None;
                }
                let hir::ExprKind::SingletonValue(value) = init.kind else {
                    return None;
                };
                Some(value)
            })
            .expect("materialized companion receiver must originate from the singleton value"),
        _ => panic!("companion method receiver must be the typed singleton value"),
    }
}

#[test]
fn companion_relation_alias_and_forwarding_are_typed() {
    let output = lower_user_output(file(vec![
        Decl::Class(host_class()),
        fun_expr(
            "forwarded",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            method_call(var("Host"), "answer", Vec::new()),
        ),
        fun_expr(
            "named",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            method_call(field(var("Host"), "Factory"), "answer", Vec::new()),
        ),
        fun_expr(
            "aliased",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            method_call(field(var("Host"), "Companion"), "answer", Vec::new()),
        ),
        fun("main", Vec::new()),
    ]))
    .expect("a generic host must own one non-generic companion singleton");

    let module = &output.export;
    let (host, _) = module
        .classes
        .iter()
        .find(|(_, declaration)| declaration.name == "Host")
        .expect("Host class");
    let (object, declaration) = module
        .objects
        .iter()
        .find(|(_, declaration)| declaration.name == "Factory")
        .expect("named companion object");
    let hir::ObjectKind::Companion(relation) = declaration.kind else {
        panic!("companion object must carry its typed relation")
    };
    let relation_declaration = &module.companion_relations[relation];
    assert_eq!(relation_declaration.host, hir::NominalOwner::Class(host));
    assert_eq!(relation_declaration.object, object);
    assert_eq!(
        relation_declaration.name,
        hir::CompanionName::Named("Factory".to_string())
    );
    assert!(
        module
            .public_surface
            .companion_relations
            .contains(&relation)
    );
    assert!(
        module.classes[host]
            .methods
            .iter()
            .all(|method| module.functions[*method].name.rsplit('.').next() != Some("answer"))
    );
    assert_eq!(
        module.classes[declaration.backing_class]
            .methods
            .iter()
            .filter(|method| {
                module.functions[**method].name.rsplit('.').next() == Some("answer")
            })
            .count(),
        1,
        "forwarding must not duplicate the companion method on its host"
    );

    let expected = declaration.singleton_value;
    for function_name in ["forwarded", "named", "aliased"] {
        let function = module
            .functions
            .iter()
            .find_map(|(_, function)| (function.name == function_name).then_some(function))
            .unwrap_or_else(|| panic!("function `{function_name}`"));
        assert_eq!(companion_receiver(function), expected);
    }

    let local_object = &output.local.objects[hir::concrete::ObjectId::from_raw(object.into_raw())];
    let hir::concrete::ObjectKind::Companion(local_relation) = local_object.kind else {
        panic!("local companion object must retain its typed relation")
    };
    let local_relation_declaration = &output.local.companion_relations[local_relation];
    assert_eq!(
        local_relation_declaration.host,
        hir::concrete::NominalOwner::Class(hir::concrete::ClassOriginId::from_raw(
            host.into_raw().into_u32()
        ))
    );
    assert_eq!(
        local_relation_declaration.object,
        hir::concrete::ObjectId::from_raw(object.into_raw())
    );
    assert!(matches!(
        local_relation_declaration.name,
        hir::concrete::CompanionName::Named(ref name) if name == "Factory"
    ));
}
