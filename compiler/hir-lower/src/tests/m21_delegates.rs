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
