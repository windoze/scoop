use super::*;

fn public_visibility() -> ast::VisibilitySyntax {
    ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Public,
        span: sp(),
    }
}

fn const_property(name: &str, value: Expr) -> ast::PropertyDecl {
    ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: ty_named("Int"),
        body: ast::PropertyBodySyntax::Const(Box::new(value)),
        span: sp(),
    }
}

fn public_object(name: &str, members: Vec<ast::ClassMember>) -> Decl {
    Decl::Object(ast::ObjectDecl {
        annotations: Vec::new(),
        visibility: public_visibility(),
        name: ident(name),
        supertypes: Vec::new(),
        members,
        span: sp(),
    })
}

fn user_function<'module>(module: &'module hir::Module, name: &str) -> &'module hir::Function {
    module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == name).then_some(function))
        .unwrap_or_else(|| panic!("function `{name}` must exist"))
}

fn function_result(function: &hir::Function) -> &hir::Expr {
    let hir::FunctionKind::User(body) = &function.kind else {
        panic!("test function must have a user body")
    };
    return_value(&body.statements)
}

#[test]
fn object_identity_chain_publish_and_default_access_are_typed() {
    let mut choose = fun_expr(
        "choose",
        Vec::new(),
        vec![("value", ty_named("Registry"))],
        Some(ty_named("Registry")),
        var("value"),
    );
    let Decl::Function(choose_declaration) = &mut choose else {
        unreachable!("function builder returns a function")
    };
    choose_declaration.visibility = public_visibility();
    choose_declaration.params[0].syntax = ast::ParameterSyntax::Default {
        expression: var("Registry"),
        equals_span: sp(),
    };

    let output = lower_user_output(file(vec![
        public_object(
            "Registry",
            vec![ast::ClassMember::StoredProperty(const_property(
                "version",
                int_lit(21),
            ))],
        ),
        choose,
        fun_expr(
            "readRegistry",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Registry")),
            var("Registry"),
        ),
        fun_expr(
            "readVersion",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            field(var("Registry"), "version"),
        ),
        fun("main", Vec::new()),
    ]))
    .expect("a public object and its default-valued use must lower");

    let module = &output.export;
    let (object_id, object) = module
        .objects
        .iter()
        .find(|(_, declaration)| declaration.name == "Registry")
        .expect("Registry object");
    let object_type = module.object_types[object.object_type];
    assert_eq!(object.kind, hir::ObjectKind::Standalone);
    let singleton = module.singleton_values[object.singleton_value];
    let published_root = &module.singleton_published_roots[singleton.published_root];
    let unit = &module.initialization_units[singleton.initialization];

    assert_eq!(object_type.declaration, object_id);
    assert_eq!(singleton.declaration, object_id);
    assert_eq!(singleton.object_type, object.object_type);
    assert_eq!(published_root.value, object.singleton_value);
    assert_eq!(published_root.ty, object_type.canonical_type);
    assert_eq!(unit.schedule, hir::InitializationSchedule::LazyAccess);
    assert!(matches!(
        unit.kind,
        hir::InitializationUnitKind::LazySingleton {
            value,
            published_root
        } if value == object.singleton_value && published_root == singleton.published_root
    ));

    let hir::FunctionKind::User(initializer) = &module.functions[unit.initializer].kind else {
        panic!("singleton initializer must be a generated user body")
    };
    assert!(matches!(
        initializer.statements.as_slice(),
        [hir::Statement {
            kind: hir::StatementKind::Assign {
                target: hir::AssignTarget::SingletonPublishedRoot(root),
                value: hir::Expr {
                    kind: hir::ExprKind::ClassInit { .. },
                    ..
                },
            },
            ..
        }] if *root == singleton.published_root
    ));
    assert!(matches!(
        function_result(user_function(module, "readRegistry")).kind,
        hir::ExprKind::SingletonValue(value) if value == object.singleton_value
    ));
    assert!(matches!(
        function_result(user_function(module, "readVersion")).kind,
        hir::ExprKind::IntLiteral(21)
    ));

    let (choose_id, _) = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "choose")
        .expect("choose function");
    let interface = module
        .source_parameter_interfaces
        .iter()
        .find(|interface| interface.owner == hir::ExportParameterOwner::Function(choose_id))
        .expect("choose source parameter interface");
    let hir::ExportParameterCalling::Default { source, .. } = interface.parameters[0].calling
    else {
        panic!("choose parameter must retain its exported default")
    };
    let template = &module.export_default_exprs[module.export_default_sources[source].expression];
    assert_eq!(template.references.singleton_values.len(), 1);
    assert_eq!(
        template.references.singleton_values[0].target,
        object.singleton_value
    );
    assert!(
        template.references.singleton_values[0]
            .witness
            .target_domain
            .is_universal()
    );

    assert!(module.public_surface.objects.contains(&object_id));
    assert!(
        module
            .public_surface
            .object_types
            .contains(&object.object_type)
    );
    assert!(
        module
            .public_surface
            .singleton_values
            .contains(&object.singleton_value)
    );
    assert!(
        !module
            .public_surface
            .classes
            .contains(&object.backing_class)
    );
    assert!(
        module.classes[object.backing_class]
            .constructors
            .iter()
            .all(|constructor| !module
                .public_surface
                .class_constructors
                .contains(constructor))
    );

    let local = &output.local;
    let (_, local_object) = local
        .objects
        .iter()
        .find(|(_, declaration)| declaration.name == "Registry")
        .expect("concrete Registry object");
    let local_singleton = local.singleton_values[local_object.singleton_value];
    let local_unit = &local.initialization_units[local_singleton.initialization];
    assert!(matches!(
        local_unit.kind,
        hir::concrete::InitializationUnitKind::LazySingleton {
            value,
            published_root
        } if value == local_object.singleton_value
            && published_root == local_singleton.published_root
    ));
    let hir::concrete::FunctionKind::User(local_initializer) =
        &local.functions[local_unit.initializer].kind
    else {
        panic!("concrete singleton initializer must be a user body")
    };
    assert!(matches!(
        local_initializer.statements.as_slice(),
        [hir::concrete::Statement {
            kind: hir::concrete::StatementKind::Assign {
                target: hir::concrete::AssignTarget::SingletonPublishedRoot(root),
                value: hir::concrete::Expr {
                    kind: hir::concrete::ExprKind::ClassNew { .. },
                    ..
                },
            },
            ..
        }] if *root == local_singleton.published_root
    ));
}
