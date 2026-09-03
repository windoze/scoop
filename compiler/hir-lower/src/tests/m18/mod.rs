use super::*;

fn operator(mut function: FunctionDecl) -> FunctionDecl {
    function.operator = Some(ast::OperatorModifier { span: sp() });
    function
}

fn infix(mut function: FunctionDecl) -> FunctionDecl {
    function.infix = Some(ast::InfixModifier { span: sp() });
    function
}

fn invoke_expr(callee: Expr, argument: Expr) -> Expr {
    Expr::Invoke {
        callee: Box::new(callee),
        type_args: Vec::new(),
        args: call_arguments(vec![argument]),
        span: sp(),
    }
}

fn direct_method_name<'a>(module: &'a hir::Module, expr: &hir::Expr) -> &'a str {
    let hir::ExprKind::MethodCall { callee, .. } = &expr.kind else {
        panic!("expected a method call, found {expr:?}");
    };
    let hir::MethodCallee::Callable(callable) = callee else {
        panic!("expected an ordinary callable method");
    };
    let function = match callable {
        hir::Callable::Function(function) => *function,
        hir::Callable::Generic(application) => {
            module.generic_functions[module.instantiations[*application].generic].function
        }
        hir::Callable::Method(application) => module.method_applications[*application].function,
        hir::Callable::GenericMethod(application) => {
            module.generic_methods[module.generic_method_applications[*application].method].function
        }
    };
    &module.functions[function].name
}

fn direct_callable_name<'a>(module: &'a hir::Module, expr: &hir::Expr) -> &'a str {
    let callee = match &expr.kind {
        hir::ExprKind::Call { callee, .. } => *callee,
        hir::ExprKind::MethodCall { callee, .. } => {
            let hir::MethodCallee::Callable(callee) = callee else {
                panic!("expected an ordinary callable method");
            };
            *callee
        }
        _ => panic!("expected a direct or method call, found {expr:?}"),
    };
    &module.functions[module.callable_function(callee)].name
}

fn operator_extension(
    receiver_ty: TypeRef,
    name: &str,
    params: Vec<(&str, TypeRef)>,
    return_ty: TypeRef,
    body: Expr,
) -> Decl {
    let Decl::Function(mut function) =
        extension_expr(receiver_ty, name, Vec::new(), params, Some(return_ty), body)
    else {
        unreachable!("extension_expr builds a function")
    };
    function.operator = Some(ast::OperatorModifier { span: sp() });
    Decl::Function(function)
}

#[test]
fn arbitrary_and_local_values_apply_operator_invoke_once() {
    let invoke = operator(method_expr(
        "invoke",
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        var("value"),
    ));
    let module = lower_user(file(vec![
        struct_decl_methods("Handler", vec![], vec![invoke]),
        fun(
            "main",
            vec![
                val("handler", call("Handler", vec![])),
                val("a", call("handler", vec![int_lit(1)])),
                val("b", invoke_expr(call("Handler", vec![]), int_lit(2))),
            ],
        ),
    ]))
    .expect("operator invoke must make nominal values callable");
    let body = function_body(&module, "main");
    assert_eq!(
        direct_method_name(&module, local_init(body, "a")),
        "Handler.invoke"
    );
    assert_eq!(
        direct_method_name(&module, local_init(body, "b")),
        "Handler.invoke"
    );
}

#[test]
fn explicit_member_call_uses_function_before_field_invoke() {
    let invoke = operator(method_expr(
        "invoke",
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        var("value"),
    ));
    let direct = method_expr(
        "handler",
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        var("value"),
    );
    let module = lower_user(file(vec![
        struct_decl_methods("Handler", vec![], vec![invoke]),
        struct_decl_methods(
            "Service",
            vec![("handler", ty_named("Handler"))],
            vec![direct],
        ),
        fun(
            "main",
            vec![
                val("service", call("Service", vec![call("Handler", vec![])])),
                val(
                    "answer",
                    method_call(var("service"), "handler", vec![int_lit(1)]),
                ),
            ],
        ),
    ]))
    .expect("member function-like layer must win before property-like invoke");
    let answer = local_init(function_body(&module, "main"), "answer");
    assert_eq!(direct_method_name(&module, answer), "Service.handler");
}

#[test]
fn member_field_then_invoke_is_a_typed_two_stage_candidate() {
    let invoke = operator(method_expr(
        "invoke",
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        var("value"),
    ));
    let module = lower_user(file(vec![
        struct_decl_methods("Handler", vec![], vec![invoke]),
        struct_decl("Service", vec![("handler", ty_named("Handler"))]),
        fun(
            "main",
            vec![
                val("service", call("Service", vec![call("Handler", vec![])])),
                val(
                    "answer",
                    method_call(var("service"), "handler", vec![int_lit(1)]),
                ),
            ],
        ),
    ]))
    .expect("a field value with operator invoke must be callable");
    let answer = local_init(function_body(&module, "main"), "answer");
    assert_eq!(direct_method_name(&module, answer), "Handler.invoke");
    assert!(body_contains_field_read(function_body(&module, "main")));
}

#[test]
fn property_like_extension_layers_follow_the_same_c_level_order() {
    let invoke_extension = operator_extension(
        ty_named("Handler"),
        "invoke",
        vec![("value", ty_named("Int"))],
        ty_named("Int"),
        var("value"),
    );
    let direct_extension = extension_expr(
        ty_named("Service"),
        "handler",
        Vec::new(),
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        var("value"),
    );
    let module = lower_user(file(vec![
        struct_decl("Handler", vec![]),
        struct_decl("Service", vec![("handler", ty_named("Handler"))]),
        invoke_extension,
        direct_extension,
        fun(
            "main",
            vec![
                val("service", call("Service", vec![call("Handler", vec![])])),
                val(
                    "direct_wins",
                    method_call(var("service"), "handler", vec![int_lit(1)]),
                ),
                val(
                    "extension_invoke",
                    invoke_expr(field(var("service"), "handler"), int_lit(2)),
                ),
            ],
        ),
    ]))
    .expect("direct extension and property extension invoke must both resolve");
    let body = function_body(&module, "main");
    assert_eq!(
        direct_callable_name(&module, local_init(body, "direct_wins")),
        "handler"
    );
    assert_eq!(
        direct_callable_name(&module, local_init(body, "extension_invoke")),
        "invoke"
    );
}

#[test]
fn current_member_property_can_be_called_without_an_explicit_this() {
    let invoke = operator(method_expr(
        "invoke",
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        var("value"),
    ));
    let run = method_expr(
        "run",
        vec![],
        Some(ty_named("Int")),
        call("handler", vec![int_lit(1)]),
    );
    let module = lower_user(file(vec![
        struct_decl_methods("Handler", vec![], vec![invoke]),
        struct_decl_methods("Host", vec![("handler", ty_named("Handler"))], vec![run]),
        fun("main", vec![]),
    ]))
    .expect("a current member property participates in property-like invoke");
    let result = return_value(&function_body(&module, "Host.run").statements);
    assert_eq!(direct_method_name(&module, result), "Handler.invoke");
}

#[test]
fn callable_local_binding_hard_shadows_but_plain_binding_does_not() {
    let invoke = operator(method_expr(
        "invoke",
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        var("value"),
    ));
    let errors = lower_user(file(vec![
        struct_decl_methods("Handler", vec![], vec![invoke.clone()]),
        fun_expr(
            "handler",
            vec![],
            vec![("value", ty_named("String"))],
            Some(ty_named("String")),
            var("value"),
        ),
        fun(
            "main",
            vec![
                val("handler", call("Handler", vec![])),
                stmt(call("handler", vec![str_lit("wrong for local")])),
            ],
        ),
    ]))
    .expect_err("an inapplicable callable binding must not fall through by name");
    assert!(errors.iter().any(|error| {
        error.message.contains("Handler.invoke") && error.message.contains("String")
    }));

    let module = lower_user(file(vec![
        struct_decl_methods("Handler", vec![], vec![invoke]),
        fun_expr(
            "handler",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            var("value"),
        ),
        fun(
            "main",
            vec![
                val("handler", int_lit(0)),
                val("answer", call("handler", vec![int_lit(1)])),
            ],
        ),
    ]))
    .expect("a non-callable local must not shadow a same-name function");
    assert_eq!(
        direct_callable_name(
            &module,
            local_init(function_body(&module, "main"), "answer")
        ),
        "handler"
    );
}

#[test]
fn function_values_outrank_extension_invoke_in_both_spellings() {
    let extension = operator_extension(
        ty_function(false, vec![ty_named("Int")], ty_named("Int")),
        "invoke",
        vec![("value", ty_named("Int"))],
        ty_named("Int"),
        var("value"),
    );
    let module = lower_user(file(vec![
        extension,
        fun_sig(
            "use",
            vec![],
            vec![(
                "function",
                ty_function(false, vec![ty_named("Int")], ty_named("Int")),
            )],
            None,
            vec![
                val("direct", call("function", vec![int_lit(1)])),
                val(
                    "explicit",
                    method_call(var("function"), "invoke", vec![int_lit(2)]),
                ),
            ],
        ),
        fun("main", vec![]),
    ]))
    .expect("function type calls must use their structural callable contract");
    let body = function_body(&module, "use");
    assert!(matches!(
        local_init(body, "direct").kind,
        hir::ExprKind::CallableCall { .. }
    ));
    assert!(matches!(
        local_init(body, "explicit").kind,
        hir::ExprKind::CallableCall { .. }
    ));
}

#[test]
fn property_like_infix_requires_operator_and_infix_and_invoke_is_not_recursive() {
    let invoke = infix(operator(method_expr(
        "invoke",
        vec![("value", ty_named("Int"))],
        Some(ty_named("Handler")),
        this_expr(),
    )));
    let infix_call = Expr::InfixCall {
        lhs: Box::new(call("Handler", vec![])),
        target: ast::InfixTarget::Invoke,
        rhs: Box::new(int_lit(1)),
        span: sp(),
    };
    let module = lower_user(file(vec![
        struct_decl_methods("Handler", vec![], vec![invoke]),
        fun("main", vec![val("once", infix_call)]),
    ]))
    .expect("operator infix invoke must support the property-like infix spelling");
    let once = local_init(function_body(&module, "main"), "once");
    let handler = module
        .structs
        .iter()
        .find(|(_, item)| item.name == "Handler")
        .map(|(_, item)| module.struct_applications[item.self_application].canonical_type)
        .expect("Handler type");
    assert_eq!(once.ty, handler);
    assert_eq!(direct_method_name(&module, once), "Handler.invoke");
}

fn body_contains_field_read(body: &hir::Body) -> bool {
    body.statements.iter().any(|statement| {
        matches!(
            &statement.kind,
            hir::StatementKind::ValDecl {
                init: hir::Expr {
                    kind: hir::ExprKind::FieldAccess { .. },
                    ..
                },
                ..
            }
        )
    })
}

#[test]
fn infix_resolution_requires_the_typed_modifier() {
    let tagged = infix(method_expr(
        "merge",
        vec![("other", ty_named("Boxed"))],
        Some(ty_named("Boxed")),
        this_expr(),
    ));
    let expression = Expr::InfixCall {
        lhs: Box::new(call("Boxed", vec![])),
        target: ast::InfixTarget::Named(ident("merge")),
        rhs: Box::new(call("Boxed", vec![])),
        span: sp(),
    };
    let module = lower_user(file(vec![
        struct_decl_methods("Boxed", vec![], vec![tagged]),
        fun("main", vec![val("result", expression)]),
    ]))
    .expect("typed infix member must resolve");
    let result = local_init(function_body(&module, "main"), "result");
    assert_eq!(direct_method_name(&module, result), "Boxed.merge");

    let errors = lower_user(file(vec![
        struct_decl_methods(
            "Plain",
            vec![],
            vec![method_expr(
                "merge",
                vec![("other", ty_named("Plain"))],
                Some(ty_named("Plain")),
                this_expr(),
            )],
        ),
        fun(
            "main",
            vec![stmt(Expr::InfixCall {
                lhs: Box::new(call("Plain", vec![])),
                target: ast::InfixTarget::Named(ident("merge")),
                rhs: Box::new(call("Plain", vec![])),
                span: sp(),
            })],
        ),
    ]))
    .expect_err("an ordinary same-name method must not acquire infix capability");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("has no infix callable `merge`"))
    );
}

fn function_body<'module>(module: &'module hir::Module, name: &str) -> &'module hir::Body {
    module
        .functions
        .iter()
        .find_map(|(_, function)| {
            (function.name == name).then(|| match &function.kind {
                hir::FunctionKind::User(body) => body,
                _ => panic!("expected user function"),
            })
        })
        .unwrap_or_else(|| panic!("missing function `{name}`"))
}
