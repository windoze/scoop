use super::*;

fn delegate_operator(mut function: FunctionDecl) -> FunctionDecl {
    function.operator = Some(ast::OperatorModifier { span: sp() });
    function
}

fn scoop_extern(name: &str) -> ast::Annotation {
    ast::Annotation {
        name: ident("Extern"),
        args: [("name", name), ("abi", "scoop")]
            .into_iter()
            .map(|(parameter, value)| ast::AnnotationArg {
                name: Some(ident(parameter)),
                value: ast::AnnotationLiteral::String(value.to_string()),
                span: sp(),
            })
            .collect(),
        span: sp(),
    }
}

fn role_method(
    name: &str,
    params: Vec<(&str, TypeRef)>,
    return_ty: Option<TypeRef>,
    body: FunctionBody,
) -> FunctionDecl {
    delegate_operator(method_full(false, false, name, params, return_ty, body))
}

fn local_delegate(name: &str, mutable: bool, ty: Option<TypeRef>, expression: Expr) -> Statement {
    Statement {
        kind: StatementKind::LocalDelegatedProperty(ast::LocalDelegatedPropertyDecl {
            mutable,
            name: ident(name),
            ty,
            expression,
            by_span: sp(),
            span: sp(),
        }),
        span: sp(),
    }
}

fn delegated_property(
    name: &str,
    mutable: bool,
    ty: TypeRef,
    expression: Expr,
) -> ast::PropertyDecl {
    ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty,
        body: ast::PropertyBodySyntax::Delegated {
            expression: Box::new(expression),
            by_span: sp(),
        },
        span: sp(),
    }
}

#[test]
fn local_delegate_is_an_immutable_hidden_local_with_resolved_get_and_set_targets() {
    let module = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "LocalDelegate",
            vec![(true, "value", ty_named("Int"))],
            None,
            Vec::new(),
            vec![
                role_method(
                    "provideDelegate",
                    Vec::new(),
                    Some(ty_named("LocalDelegate")),
                    FunctionBody::Expr(Box::new(var("this"))),
                ),
                role_method(
                    "getValue",
                    vec![("thisRef", ty_named("Unit"))],
                    Some(ty_named("Int")),
                    FunctionBody::Expr(Box::new(var("value"))),
                ),
                role_method(
                    "setValue",
                    vec![("thisRef", ty_named("Unit")), ("next", ty_named("Int"))],
                    None,
                    FunctionBody::Block(block(vec![assign("value", var("next"))])),
                ),
            ],
        ),
        fun(
            "main",
            vec![
                local_delegate(
                    "number",
                    true,
                    None,
                    call("LocalDelegate", vec![int_lit(1)]),
                ),
                val("observed", var("number")),
                assign("number", int_lit(2)),
            ],
        ),
    ]))
    .expect("a local delegated var lowers");

    let main = module.entry;
    let hir::FunctionKind::User(body) = &module.functions[main].kind else {
        panic!("main is a user function")
    };
    let hidden = body
        .locals
        .iter()
        .find(|(_, local)| local.name.starts_with("$delegate."))
        .expect("local delegate has hidden storage");
    assert!(!hidden.1.mutable);
    assert!(matches!(
        local_init(body, "observed").kind,
        hir::ExprKind::MethodCall { .. }
    ));
    assert!(body.statements.iter().any(|statement| matches!(
        statement.kind,
        hir::StatementKind::Expr(hir::Expr {
            kind: hir::ExprKind::MethodCall { .. },
            ..
        })
    )));
}

#[test]
fn class_delegate_owns_typed_hidden_storage_and_generated_accessors() {
    let mut host_declaration = class_decl(
        ast::ClassModifier::Final,
        "Host",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let Decl::Class(host) = &mut host_declaration else {
        unreachable!("class_decl constructs a class")
    };
    host.members
        .push(ast::ClassMember::StoredProperty(delegated_property(
            "number",
            true,
            ty_named("Int"),
            call("OwnerDelegate", vec![int_lit(1)]),
        )));

    let module = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "OwnerDelegate",
            vec![(true, "value", ty_named("Int"))],
            None,
            Vec::new(),
            vec![
                role_method(
                    "getValue",
                    vec![("thisRef", ty_named("Host"))],
                    Some(ty_named("Int")),
                    FunctionBody::Expr(Box::new(var("value"))),
                ),
                role_method(
                    "setValue",
                    vec![("thisRef", ty_named("Host")), ("next", ty_named("Int"))],
                    None,
                    FunctionBody::Block(block(vec![assign("value", var("next"))])),
                ),
            ],
        ),
        host_declaration,
        fun("main", Vec::new()),
    ]))
    .expect("a class delegated property lowers");

    let (_, host) = module
        .classes
        .iter()
        .find(|(_, class)| class.name == "Host")
        .expect("Host class");
    let property = host.properties[0];
    let hir::PropertyRepresentation::Delegated { storage } =
        module.properties[property].representation
    else {
        panic!("class delegate has a delegated representation")
    };
    let hir::DelegateStorageLocation::ClassField(field) =
        module.delegate_storages[storage].location;
    assert_eq!(host.fields, [field]);
    let constructor = host.constructors[0];
    let hir::ClassConstructorKind::Primary {
        common_initialization,
        ..
    } = &module.class_constructors[constructor].kind
    else {
        panic!("Host has a primary constructor")
    };
    assert!(matches!(
        common_initialization.as_slice(),
        [hir::ClassInitializationStep::DelegatedProperty {
            storage: actual_storage,
            field: actual_field,
            ..
        }] if *actual_storage == storage && *actual_field == field
    ));
    let getter = module.properties[property].capability.getter();
    let hir::PropertyAccessorImplementation::Body(getter) =
        module.property_getters[getter].implementation
    else {
        panic!("delegated getter is generated as a body")
    };
    let hir::FunctionKind::User(body) = &module.functions[getter].kind else {
        panic!("delegated getter has a user body")
    };
    assert!(matches!(
        return_value(&body.statements).kind,
        hir::ExprKind::MethodCall { .. }
    ));
}

#[test]
fn property_delegate_operators_have_disjoint_typed_roles() {
    let module = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Delegate",
            Vec::new(),
            None,
            Vec::new(),
            vec![
                role_method(
                    "provideDelegate",
                    Vec::new(),
                    Some(ty_named("Delegate")),
                    FunctionBody::Expr(Box::new(var("this"))),
                ),
                role_method(
                    "getValue",
                    vec![("thisRef", ty_named("Unit"))],
                    Some(ty_named("Int")),
                    FunctionBody::Expr(Box::new(int_lit(1))),
                ),
                role_method(
                    "setValue",
                    vec![("thisRef", ty_named("Unit")), ("value", ty_named("Int"))],
                    None,
                    FunctionBody::Block(block(Vec::new())),
                ),
            ],
        ),
        interface_decl(
            "Readable",
            vec![delegate_operator(bodyless_method(
                false,
                "getValue",
                vec![("thisRef", ty_named("Unit"))],
                Some(ty_named("Int")),
            ))],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "Ordinary",
            Vec::new(),
            None,
            Vec::new(),
            vec![method_expr(
                "getValue",
                vec![("thisRef", ty_named("Unit"))],
                Some(ty_named("Int")),
                int_lit(0),
            )],
        ),
        fun("main", Vec::new()),
    ]))
    .expect("valid property delegate roles lower");

    let roles = module
        .functions
        .iter()
        .filter(|(_, function)| function.name.starts_with("Delegate."))
        .map(|(_, function)| function.modifiers.property_delegate_operator)
        .collect::<Vec<_>>();
    assert_eq!(
        roles,
        vec![
            Some(hir::PropertyDelegateOperatorKind::ProvideDelegate),
            Some(hir::PropertyDelegateOperatorKind::GetValue),
            Some(hir::PropertyDelegateOperatorKind::SetValue),
        ]
    );
    assert!(module.functions.iter().all(|(_, function)| {
        function.modifiers.property_delegate_operator.is_none()
            || function.modifiers.operator.is_none()
    }));
    let dump = hir::dump(&module);
    assert!(
        dump.contains("operator fun getValue(thisRef: Unit): Int"),
        "{dump}"
    );
}

#[test]
fn property_delegate_operator_shapes_are_rejected_at_declaration() {
    let mut default_get = role_method(
        "getValue",
        vec![("thisRef", ty_named("Unit"))],
        Some(ty_named("Int")),
        FunctionBody::Expr(Box::new(int_lit(0))),
    );
    default_get.params[0].syntax = ast::ParameterSyntax::Default {
        expression: int_lit(0),
        equals_span: sp(),
    };
    let mut suspended = role_method(
        "provideDelegate",
        Vec::new(),
        Some(ty_named("SuspendRole")),
        FunctionBody::Expr(Box::new(var("this"))),
    );
    suspended.is_suspend = true;
    let mut generic = role_method(
        "getValue",
        vec![("thisRef", ty_named("Unit"))],
        Some(ty_named("T")),
        FunctionBody::Expr(Box::new(int_lit(0))),
    );
    generic.type_params = vec![type_param("T")];
    let Decl::Function(mut external) = extension_expr(
        ty_named("Int"),
        "getValue",
        Vec::new(),
        vec![("thisRef", ty_named("Unit"))],
        Some(ty_named("Int")),
        int_lit(0),
    ) else {
        unreachable!("extension_expr builds a function")
    };
    external.operator = Some(ast::OperatorModifier { span: sp() });
    external.annotations = vec![scoop_extern("native_get_value")];
    external.body = FunctionBody::None;

    let errors = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "WrongProvide",
            Vec::new(),
            None,
            Vec::new(),
            vec![role_method(
                "provideDelegate",
                vec![("extra", ty_named("Int"))],
                Some(ty_named("WrongProvide")),
                FunctionBody::Expr(Box::new(var("this"))),
            )],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "WrongGet",
            Vec::new(),
            None,
            Vec::new(),
            vec![default_get],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "SuspendRole",
            Vec::new(),
            None,
            Vec::new(),
            vec![suspended],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "GenericRole",
            Vec::new(),
            None,
            Vec::new(),
            vec![generic],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "WrongSet",
            Vec::new(),
            None,
            Vec::new(),
            vec![role_method(
                "setValue",
                vec![("thisRef", ty_named("Unit")), ("value", ty_named("Int"))],
                Some(ty_named("Int")),
                FunctionBody::Expr(Box::new(int_lit(0))),
            )],
        ),
        Decl::Function(external),
        fun("main", Vec::new()),
    ]))
    .expect_err("malformed property delegate roles are rejected");

    for expected in [
        "property delegate operator `provideDelegate` must have exactly 0 parameters, found 1",
        "property delegate operator `getValue` requires required, non-vararg parameters",
        "property delegate operator `provideDelegate` must not be suspend",
        "property delegate operator `getValue` must not declare type parameters",
        "property delegate operator `setValue` must return Unit, found Int",
        "property delegate operator `getValue` must be an ordinary function",
    ] {
        assert!(
            errors.iter().any(|error| error.message == expected),
            "missing diagnostic: {expected}; found {errors:?}"
        );
    }
}

#[test]
fn delegate_role_is_part_of_the_interface_method_contract() {
    let interface_get = delegate_operator(bodyless_method(
        false,
        "getValue",
        vec![("thisRef", ty_named("Unit"))],
        Some(ty_named("Int")),
    ));
    let plain_get = method_full(
        true,
        false,
        "getValue",
        vec![("thisRef", ty_named("Unit"))],
        Some(ty_named("Int")),
        FunctionBody::Expr(Box::new(int_lit(0))),
    );
    let errors = lower_user(file(vec![
        interface_decl("ReadableDelegate", vec![interface_get]),
        class_decl(
            ast::ClassModifier::Final,
            "Plain",
            Vec::new(),
            None,
            vec!["ReadableDelegate"],
            vec![plain_get],
        ),
        fun("main", Vec::new()),
    ]))
    .expect_err("an ordinary same-name method does not implement a delegate role");
    assert!(errors.iter().any(|error| {
        error.message
            == "`getValue` must have the same `operator` modifier as `ReadableDelegate.getValue`"
    }));
}
