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

fn multi_index(receiver: Expr, indices: Vec<Expr>) -> Expr {
    let mut indices = indices.into_iter();
    Expr::Index {
        receiver: Box::new(receiver),
        indices: ast::NonEmptyVec::new(
            indices.next().expect("test index must be non-empty"),
            indices.collect(),
        ),
        span: sp(),
    }
}

fn assignment(target: ast::PlaceExpr, op: ast::AssignmentOp, value: Expr) -> Statement {
    Statement {
        kind: StatementKind::Assign(ast::Assign {
            target,
            op,
            value,
            span: sp(),
        }),
        span: sp(),
    }
}

fn index_place(receiver: Expr, indices: Vec<Expr>) -> ast::PlaceExpr {
    let mut indices = indices.into_iter();
    ast::PlaceExpr::Index {
        receiver: Box::new(receiver),
        indices: ast::NonEmptyVec::new(
            indices.next().expect("test index must be non-empty"),
            indices.collect(),
        ),
        span: sp(),
    }
}

fn update(place: ast::PlaceExpr, op: ast::UpdateOp, notation: ast::UpdateNotation) -> Expr {
    Expr::Update {
        place,
        op,
        notation,
        span: sp(),
    }
}

fn safe_method_call(receiver: Expr, name: &str, args: Vec<Expr>) -> Expr {
    Expr::MethodCall {
        receiver: Box::new(receiver),
        name: ident(name),
        navigation: ast::Navigation::Safe,
        type_args: Vec::new(),
        args: call_arguments(args),
        span: sp(),
    }
}

fn int_identity_lambda() -> Expr {
    Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters: Some(vec![ast::LambdaParam {
            target: pat_bind("value"),
            ty: None,
            span: sp(),
        }]),
        body: block(vec![stmt(var("value"))]),
        span: sp(),
    }
}

fn direct_method_name<'a>(module: &'a hir::Module, expr: &hir::Expr) -> &'a str {
    let hir::ExprKind::MethodCall { callee, .. } = &expr.kind else {
        panic!("expected a method call, found {expr:?}");
    };
    let hir::MethodCallee::Callable(hir::CallableTarget::Local(callable)) = callee else {
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
        hir::ExprKind::Call {
            callee: hir::CallableTarget::Local(callee),
            ..
        } => *callee,
        hir::ExprKind::MethodCall { callee, .. } => {
            let hir::MethodCallee::Callable(hir::CallableTarget::Local(callee)) = callee else {
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

#[test]
fn overloadable_expressions_select_only_typed_operator_roles() {
    let unary_minus = operator(method_expr(
        "unaryMinus",
        vec![],
        Some(ty_named("Number")),
        this_expr(),
    ));
    let plus = operator(method_expr(
        "plus",
        vec![("other", ty_named("Number"))],
        Some(ty_named("Number")),
        this_expr(),
    ));
    let range_to = operator(method_expr(
        "rangeTo",
        vec![("other", ty_named("Number"))],
        Some(ty_named("Number")),
        this_expr(),
    ));
    let compare_to = operator(method_expr(
        "compareTo",
        vec![("other", ty_named("Number"))],
        Some(ty_named("Long")),
        int_lit(0),
    ));
    let contains = operator(method_expr(
        "contains",
        vec![("item", ty_named("Number"))],
        Some(ty_named("Boolean")),
        bool_lit(true),
    ));
    let times = operator_extension(
        ty_named("Number"),
        "times",
        vec![("scale", ty_named("Int"))],
        ty_named("Number"),
        this_expr(),
    );
    let module = lower_user(file(vec![
        struct_decl_methods(
            "Number",
            vec![],
            vec![unary_minus, plus, range_to, compare_to],
        ),
        struct_decl_methods("Bag", vec![], vec![contains]),
        times,
        fun(
            "main",
            vec![
                val("negated", unary(UnOp::Neg, call("Number", vec![]))),
                val(
                    "sum",
                    binary(BinOp::Add, call("Number", vec![]), call("Number", vec![])),
                ),
                val(
                    "scaled",
                    binary(BinOp::Mul, call("Number", vec![]), int_lit(2)),
                ),
                val(
                    "range",
                    binary(
                        BinOp::RangeTo,
                        call("Number", vec![]),
                        call("Number", vec![]),
                    ),
                ),
                val(
                    "ordered",
                    binary(BinOp::Lt, call("Number", vec![]), call("Number", vec![])),
                ),
                val(
                    "present",
                    binary(BinOp::Contains, call("Number", vec![]), call("Bag", vec![])),
                ),
                val(
                    "absent",
                    binary(
                        BinOp::NotContains,
                        call("Number", vec![]),
                        call("Bag", vec![]),
                    ),
                ),
                val("remainder", binary(BinOp::Rem, int_lit(7), int_lit(3))),
            ],
        ),
    ]))
    .expect("every mapped expression must use its typed operator role");
    let body = function_body(&module, "main");
    assert_eq!(
        direct_method_name(&module, local_init(body, "negated")),
        "Number.unaryMinus"
    );
    assert_eq!(
        direct_method_name(&module, local_init(body, "sum")),
        "Number.plus"
    );
    assert_eq!(
        direct_callable_name(&module, local_init(body, "scaled")),
        "times"
    );
    assert_eq!(
        direct_method_name(&module, local_init(body, "range")),
        "Number.rangeTo"
    );
    let hir::ExprKind::Binary {
        lhs: comparison, ..
    } = &local_init(body, "ordered").kind
    else {
        panic!("comparison must compare the typed compareTo result with zero")
    };
    assert_eq!(direct_method_name(&module, comparison), "Number.compareTo");
    assert_eq!(
        direct_method_name(&module, local_init(body, "present")),
        "Bag.contains"
    );
    let hir::ExprKind::Unary { operand, .. } = &local_init(body, "absent").kind else {
        panic!("!in must negate the exact Boolean contains result")
    };
    assert_eq!(direct_method_name(&module, operand), "Bag.contains");
    assert!(matches!(
        local_init(body, "remainder").kind,
        hir::ExprKind::IntegerOperation {
            operation: hir::IntegerOperation::Managed {
                kind: hir::IntegerKind::SIGNED_32,
                operation: hir::IntegerDivRem::Rem,
                ..
            },
            ..
        }
    ));
}

#[test]
fn core_operator_winners_normalize_to_closed_typed_intrinsics() {
    let module = lower_user(file(vec![fun(
        "main",
        vec![
            val("positive", unary(UnOp::Plus, int_lit(1))),
            val("negative", unary(UnOp::Neg, int_lit(1))),
            val("sum", binary(BinOp::Add, int_lit(1), int_lit(2))),
            val("difference", binary(BinOp::Sub, int_lit(3), int_lit(2))),
            val("product", binary(BinOp::Mul, int_lit(3), int_lit(2))),
            val("quotient", binary(BinOp::Div, int_lit(6), int_lit(2))),
            val("remainder", binary(BinOp::Rem, int_lit(7), int_lit(3))),
            val("equal", binary(BinOp::Eq, int_lit(1), int_lit(1))),
            val("ordered", binary(BinOp::Lt, int_lit(1), int_lit(2))),
            val("negated", unary(UnOp::Not, bool_lit(false))),
            val("text", binary(BinOp::Add, str_lit("a"), str_lit("b"))),
            val(
                "text_ordered",
                binary(BinOp::Lt, str_lit("a"), str_lit("b")),
            ),
            val("array", array_lit(vec![int_lit(1)])),
            val("first", subscript(var("array"), int_lit(0))),
            val_ty(
                "mutable",
                Some(ty_generic("MutableArray", vec![ty_named("Int")])),
                array_lit(vec![int_lit(1)]),
            ),
            assign_index(var("mutable"), int_lit(0), int_lit(2)),
        ],
    )]))
    .expect("validated core operators must normalize after overload selection");
    let body = function_body(&module, "main");
    assert!(matches!(
        local_init(body, "positive").kind,
        hir::ExprKind::IntegerOperation {
            operation: hir::IntegerOperation::NoGc {
                kind: hir::IntegerKind::SIGNED_32,
                operation: hir::NoGcIntegerOperation::UnaryPlus,
                ..
            },
            ..
        }
    ));
    assert!(matches!(
        local_init(body, "negative").kind,
        hir::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed32(value))
            if value == u32::MAX
    ));
    assert!(matches!(
        local_init(body, "negated").kind,
        hir::ExprKind::PrimitiveUnary {
            kind: hir::PrimitiveUnaryKind::BooleanNot,
            ..
        }
    ));
    for (name, operation) in [
        ("sum", hir::NoGcIntegerOperation::Add),
        ("difference", hir::NoGcIntegerOperation::Sub),
        ("product", hir::NoGcIntegerOperation::Mul),
    ] {
        assert!(matches!(
            &local_init(body, name).kind,
            hir::ExprKind::IntegerOperation {
                operation: hir::IntegerOperation::NoGc {
                    kind: hir::IntegerKind::SIGNED_32,
                    operation: actual,
                    ..
                },
                ..
            } if *actual == operation
        ));
    }
    assert!(matches!(
        local_init(body, "equal").kind,
        hir::ExprKind::IntegerOperation {
            operation: hir::IntegerOperation::NoGc {
                kind: hir::IntegerKind::SIGNED_32,
                operation: hir::NoGcIntegerOperation::Equals,
                ..
            },
            ..
        }
    ));
    for (name, operation) in [
        ("quotient", hir::IntegerDivRem::Div),
        ("remainder", hir::IntegerDivRem::Rem),
    ] {
        assert!(matches!(
            &local_init(body, name).kind,
            hir::ExprKind::IntegerOperation {
                operation: hir::IntegerOperation::Managed {
                    kind: hir::IntegerKind::SIGNED_32,
                    operation: actual,
                    ..
                },
                ..
            } if *actual == operation
        ));
    }
    assert!(matches!(
        local_init(body, "text").kind,
        hir::ExprKind::PrimitiveBinary {
            kind: hir::PrimitiveBinaryKind::StringConcat,
            ..
        }
    ));
    let hir::ExprKind::Binary { lhs, .. } = &local_init(body, "ordered").kind else {
        panic!("comparison must compare a typed compareTo result")
    };
    assert!(matches!(
        &lhs.kind,
        hir::ExprKind::IntegerOperation {
            operation: hir::IntegerOperation::NoGc {
                kind: hir::IntegerKind::SIGNED_32,
                operation: hir::NoGcIntegerOperation::CompareTo,
                ..
            },
            ..
        }
    ));
    let hir::ExprKind::Binary { lhs, .. } = &local_init(body, "text_ordered").kind else {
        panic!("comparison must compare a typed compareTo result")
    };
    assert!(matches!(
        &lhs.kind,
        hir::ExprKind::PrimitiveBinary {
            kind: hir::PrimitiveBinaryKind::StringCompareTo,
            ..
        }
    ));
    assert!(matches!(
        local_init(body, "first").kind,
        hir::ExprKind::Index {
            access: hir::ArrayAccessKind::ImmutableGet,
            ..
        }
    ));
    assert!(body.statements.iter().any(|statement| matches!(
        statement.kind,
        hir::StatementKind::Expr(hir::Expr {
            kind: hir::ExprKind::ArraySet {
                access: hir::ArrayAccessKind::MutableSet,
                ..
            },
            ..
        })
    )));
    let dump = hir::dump(&module);
    assert!(!dump.contains("MethodCall Int.plus"), "{dump}");
    assert!(!dump.contains("MethodCall String.compareTo"), "{dump}");
}

#[test]
fn core_operator_intrinsics_are_required_and_shape_checked() {
    let user = || file(vec![fun("main", vec![])]);

    let mut missing = core_file();
    let int = missing
        .declarations
        .iter_mut()
        .find_map(|declaration| match declaration {
            Decl::Struct(strukt) if strukt.name.text == "Int" => Some(strukt),
            _ => None,
        })
        .expect("test core declares Int");
    int.members.retain(|member| {
        !matches!(member, ast::StructMember::Function(method) if method.name.text == "plus")
    });
    let errors = lower(&[missing, user()]).expect_err("Int.plus is a required core contract");
    assert!(errors.iter().any(|error| {
        error.message == "scoop.core must define exactly one `int_add` intrinsic"
    }));

    let mut malformed = core_file();
    let string = malformed
        .declarations
        .iter_mut()
        .find_map(|declaration| match declaration {
            Decl::Class(class) if class.name.text == "String" => Some(class),
            _ => None,
        })
        .expect("test core declares String");
    string
        .members
        .iter_mut()
        .filter_map(|member| match member {
            ast::ClassMember::Function(method) => Some(method),
            _ => None,
        })
        .find(|method| method.name.text == "compareTo")
        .expect("test core declares String.compareTo")
        .operator = None;
    let errors =
        lower(&[malformed, user()]).expect_err("String.compareTo has an exact intrinsic signature");
    assert!(
        errors.iter().any(|error| {
            error.message == "malformed core operator intrinsic `string_compare_to`"
        })
    );
}

#[test]
fn operator_argument_materialization_stays_on_the_short_circuit_rhs() {
    let module = lower_user(file(vec![
        fun_expr("next", vec![], vec![], Some(ty_named("Int")), int_lit(1)),
        fun(
            "main",
            vec![val(
                "result",
                binary(
                    BinOp::And,
                    bool_lit(false),
                    binary(BinOp::Gt, call("next", vec![]), int_lit(0)),
                ),
            )],
        ),
    ]))
    .expect("RHS operator calls must preserve Boolean short-circuiting");
    let body = function_body(&module, "main");
    let hir::StatementKind::If {
        then_body,
        else_body: Some(_),
        ..
    } = &body.statements[0].kind
    else {
        panic!("operator setup must be represented by a short-circuit branch")
    };
    assert!(then_body.iter().any(|statement| match &statement.kind {
        hir::StatementKind::ValDecl { init, .. }
            if matches!(init.kind, hir::ExprKind::Call { .. }) =>
        {
            direct_callable_name(&module, init) == "next"
        }
        _ => false,
    }));
    assert!(
        !body.statements[1..]
            .iter()
            .any(|statement| match &statement.kind {
                hir::StatementKind::ValDecl { init, .. }
                    if matches!(init.kind, hir::ExprKind::Call { .. }) =>
                {
                    direct_callable_name(&module, init) == "next"
                }
                _ => false,
            })
    );
}

#[test]
fn ordinary_same_name_function_does_not_gain_an_operator_role() {
    let plain = method_expr(
        "plus",
        vec![("other", ty_named("Plain"))],
        Some(ty_named("Plain")),
        this_expr(),
    );
    let errors = lower_user(file(vec![
        struct_decl_methods("Plain", vec![], vec![plain]),
        fun(
            "main",
            vec![stmt(binary(
                BinOp::Add,
                call("Plain", vec![]),
                call("Plain", vec![]),
            ))],
        ),
    ]))
    .expect_err("a matching source name is not an operator capability");
    assert!(
        errors
            .iter()
            .any(|error| { error.message == "type `Plain` has no method `plus`" })
    );
}

#[test]
fn declaration_validation_rejects_incomplete_operator_and_infix_contracts() {
    let get = operator(method_expr(
        "get",
        vec![],
        Some(ty_named("Int")),
        int_lit(0),
    ));
    let set = operator(method_expr(
        "set",
        vec![("index", ty_named("Int"))],
        None,
        unit_lit(),
    ));
    let contains = operator(method_expr(
        "contains",
        vec![("item", ty_named("Int"))],
        Some(ty_named("Int")),
        int_lit(0),
    ));
    let inc = operator(method_expr(
        "inc",
        vec![],
        Some(ty_named("Other")),
        call("Other", vec![]),
    ));
    let mut invalid_set_vararg = operator(method_expr(
        "set",
        vec![("index", ty_named("Int")), ("value", ty_named("Int"))],
        None,
        unit_lit(),
    ));
    invalid_set_vararg.params[1].syntax = ast::ParameterSyntax::Vararg {
        modifier_span: sp(),
        default: ast::VarargDefaultSyntax::EmptyWhenOmitted,
    };
    let mut invalid_infix = infix(method_expr(
        "merge",
        vec![("other", ty_named("Broken"))],
        Some(ty_named("Broken")),
        this_expr(),
    ));
    invalid_infix.params[0].syntax = ast::ParameterSyntax::Default {
        expression: call("Broken", vec![]),
        equals_span: sp(),
    };
    let component_zero = operator(method_expr(
        "component0",
        vec![],
        Some(ty_named("Int")),
        int_lit(0),
    ));
    let component_overflow = operator(method_expr(
        "component4294967296",
        vec![],
        Some(ty_named("Int")),
        int_lit(0),
    ));
    let errors = lower_user(file(vec![
        struct_decl("Other", vec![]),
        struct_decl_methods(
            "Broken",
            vec![],
            vec![
                get,
                set,
                contains,
                inc,
                invalid_set_vararg,
                invalid_infix,
                component_zero,
                component_overflow,
            ],
        ),
        fun("main", vec![]),
    ]))
    .expect_err("invalid callable modifiers must be rejected in signature lowering");
    for expected in [
        "operator `get` must have at least one parameter",
        "operator `set` must have at least two parameters, found 1",
        "operator `contains` must return Boolean, found Int",
        "operator `inc` must return a subtype of receiver type Broken, found Other",
        "operator `set` value parameter must not be `vararg`",
        "infix function `merge` requires one required, non-vararg parameter",
        "operator `component0` is not valid",
        "operator `component4294967296` must use a positive decimal component index",
    ] {
        assert!(
            errors.iter().any(|error| error.message == expected),
            "missing diagnostic {expected:?}; found {errors:?}"
        );
    }
}

#[test]
fn component_and_iterator_roles_are_exported_as_typed_identities() {
    let component = operator(method_expr(
        "component12",
        vec![],
        Some(ty_named("Int")),
        int_lit(12),
    ));
    let iterator = operator(method_expr(
        "iterator",
        vec![],
        Some(ty_named("Cursor")),
        call("Cursor", vec![]),
    ));
    let module = lower_user(file(vec![
        struct_decl("Cursor", vec![]),
        struct_decl_methods("Sequence", vec![], vec![component, iterator]),
        fun("main", vec![]),
    ]))
    .expect("conventional roles not consumed until later milestones still cross HIR");
    let roles = module
        .functions
        .iter()
        .filter(|(_, function)| {
            matches!(
                function.name.as_str(),
                "Sequence.component12" | "Sequence.iterator"
            )
        })
        .map(|(_, function)| function.modifiers.operator)
        .collect::<Vec<_>>();
    assert_eq!(
        roles,
        vec![
            Some(hir::OperatorKind::Component {
                index: std::num::NonZeroU32::new(12).expect("nonzero")
            }),
            Some(hir::OperatorKind::Iterator),
        ]
    );
}

#[test]
fn class_destructuring_uses_typed_components_and_materializes_subject_once() {
    let first = operator(method_expr(
        "component1",
        vec![],
        Some(ty_named("Int")),
        int_lit(1),
    ));
    let second = operator(method_expr(
        "component2",
        vec![],
        Some(ty_named("String")),
        str_lit("second"),
    ));
    let module = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Pair",
            vec![],
            None,
            vec![],
            vec![first, second],
        ),
        fun(
            "main",
            vec![val_pat(
                false,
                pat_tuple(vec![pat_bind("left"), pat_bind("right")], None),
                None,
                call("Pair", vec![]),
            )],
        ),
    ]))
    .expect("class positional destructuring must resolve component roles");
    let body = function_body(&module, "main");
    assert_eq!(
        body.statements
            .iter()
            .filter(|statement| matches!(
                statement.kind,
                hir::StatementKind::ValDecl {
                    init: hir::Expr {
                        kind: hir::ExprKind::ClassInit { .. },
                        ..
                    },
                    ..
                }
            ))
            .count(),
        1,
        "the destructuring subject must be evaluated once"
    );
    let component_calls = body
        .statements
        .iter()
        .filter_map(|statement| {
            let hir::StatementKind::ValDecl { init, .. } = &statement.kind else {
                return None;
            };
            matches!(init.kind, hir::ExprKind::MethodCall { .. })
                .then(|| direct_method_name(&module, init))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        component_calls,
        vec!["Pair.component1", "Pair.component2"],
        "component actions execute in source order"
    );
    for name in ["left", "right"] {
        let hir::ExprKind::Local(component) = local_init(body, name).kind else {
            panic!("a user leaf must read its completed component temporary")
        };
        assert!(
            body.locals[component]
                .name
                .starts_with("$binding.component.")
        );
        assert!(!body.locals[component].mutable);
    }

    let errors = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Plain",
            vec![],
            None,
            vec![],
            vec![method_expr(
                "component1",
                vec![],
                Some(ty_named("Int")),
                int_lit(1),
            )],
        ),
        fun(
            "main",
            vec![val_pat(
                false,
                pat_tuple(vec![pat_bind("value")], None),
                None,
                call("Plain", vec![]),
            )],
        ),
    ]))
    .expect_err("an ordinary component-like name must not enable class destructuring");
    assert!(
        errors
            .iter()
            .any(|error| error.message == "type `Plain` has no method `component1`")
    );
}

#[test]
fn class_destructuring_resolves_only_the_exact_written_component_prefix() {
    let sparse = || {
        class_decl(
            ast::ClassModifier::Final,
            "Sparse",
            vec![],
            None,
            vec![],
            vec![
                operator(method_expr(
                    "component1",
                    vec![],
                    Some(ty_named("Int")),
                    int_lit(1),
                )),
                operator(method_expr(
                    "component3",
                    vec![],
                    Some(ty_named("Int")),
                    int_lit(3),
                )),
            ],
        )
    };
    let module = lower_user(file(vec![
        sparse(),
        fun(
            "main",
            vec![val_pat(
                false,
                pat_tuple(vec![pat_bind("first")], None),
                None,
                call("Sparse", vec![]),
            )],
        ),
    ]))
    .expect("one written position requires only component1");
    let body = function_body(&module, "main");
    let calls = body
        .statements
        .iter()
        .filter_map(|statement| {
            let hir::StatementKind::ValDecl { init, .. } = &statement.kind else {
                return None;
            };
            matches!(init.kind, hir::ExprKind::MethodCall { .. })
                .then(|| direct_method_name(&module, init))
        })
        .collect::<Vec<_>>();
    assert_eq!(calls, vec!["Sparse.component1"]);

    let errors = lower_user(file(vec![
        sparse(),
        fun(
            "main",
            vec![val_pat(
                false,
                pat_tuple(vec![pat_bind("first"), pat_bind("second")], None),
                None,
                call("Sparse", vec![]),
            )],
        ),
    ]))
    .expect_err("the second written position requires component2 exactly");
    assert!(
        errors
            .iter()
            .any(|error| error.message == "type `Sparse` has no method `component2`")
    );

    let errors = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Offset",
            vec![],
            None,
            vec![],
            vec![operator(method_expr(
                "component2",
                vec![],
                Some(ty_named("Int")),
                int_lit(2),
            ))],
        ),
        fun(
            "main",
            vec![val_pat(
                false,
                pat_tuple(vec![pat_bind("value")], None),
                None,
                call("Offset", vec![]),
            )],
        ),
    ]))
    .expect_err("component2 does not imply component1");
    assert!(
        errors
            .iter()
            .any(|error| error.message == "type `Offset` has no method `component1`")
    );
}

#[test]
fn class_destructuring_is_depth_first_calls_wildcards_and_rejects_rest() {
    let inner = class_decl(
        ast::ClassModifier::Final,
        "Inner",
        vec![],
        None,
        vec![],
        vec![
            operator(method_expr(
                "component1",
                vec![],
                Some(ty_named("Int")),
                int_lit(1),
            )),
            operator(method_expr(
                "component2",
                vec![],
                Some(ty_named("Int")),
                int_lit(2),
            )),
        ],
    );
    let outer = class_decl(
        ast::ClassModifier::Final,
        "Outer",
        vec![],
        None,
        vec![],
        vec![
            operator(method_expr(
                "component1",
                vec![],
                Some(ty_named("Inner")),
                call("Inner", vec![]),
            )),
            operator(method_expr(
                "component2",
                vec![],
                Some(ty_named("Int")),
                int_lit(3),
            )),
        ],
    );
    let module = lower_user(file(vec![
        inner,
        outer,
        fun(
            "main",
            vec![val_pat(
                true,
                pat_tuple(
                    vec![
                        pat_tuple(vec![pat_bind("left"), pat_wild()], None),
                        pat_bind("right"),
                    ],
                    None,
                ),
                None,
                call("Outer", vec![]),
            )],
        ),
    ]))
    .expect("nested class components must use one depth-first plan");
    let body = function_body(&module, "main");
    let calls = body
        .statements
        .iter()
        .filter_map(|statement| {
            let hir::StatementKind::ValDecl { init, .. } = &statement.kind else {
                return None;
            };
            matches!(init.kind, hir::ExprKind::MethodCall { .. })
                .then(|| direct_method_name(&module, init))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        calls,
        vec![
            "Outer.component1",
            "Inner.component1",
            "Inner.component2",
            "Outer.component2",
        ],
        "a wildcard still invokes its component and nested actions are depth-first"
    );
    for (_, local) in body.locals.iter() {
        if local.name.starts_with("$binding.") {
            assert!(!local.mutable, "all plan temporaries are immutable");
        }
        if matches!(local.name.as_str(), "left" | "right") {
            assert!(local.mutable, "only var leaves are mutable");
        }
    }

    let errors = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Single",
            vec![],
            None,
            vec![],
            vec![operator(method_expr(
                "component1",
                vec![],
                Some(ty_named("Int")),
                int_lit(1),
            ))],
        ),
        fun(
            "main",
            vec![val_pat(
                false,
                pat_tuple(vec![pat_bind("value")], Some(sp())),
                None,
                call("Single", vec![]),
            )],
        ),
    ]))
    .expect_err("class component patterns have no declaration-sized rest");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(
        errors[0].message,
        "a class component pattern cannot contain `..`"
    );
}

#[test]
fn class_binding_plan_keeps_one_complete_lambda_parameter() {
    let operation = ast::Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters: Some(vec![ast::LambdaParam {
            target: pat_tuple(vec![pat_bind("left"), pat_wild()], None),
            ty: None,
            span: sp(),
        }]),
        body: block(vec![stmt(var("left"))]),
        span: sp(),
    };
    let module = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Pair",
            vec![],
            None,
            vec![],
            vec![
                operator(method_expr(
                    "component1",
                    vec![],
                    Some(ty_named("Int")),
                    int_lit(1),
                )),
                operator(method_expr(
                    "component2",
                    vec![],
                    Some(ty_named("String")),
                    str_lit("ignored"),
                )),
            ],
        ),
        fun(
            "main",
            vec![val_ty(
                "operation",
                Some(ty_function(false, vec![ty_named("Pair")], ty_named("Int"))),
                operation,
            )],
        ),
    ]))
    .expect("a lambda class pattern must share the irrefutable planner");
    let (_, lambda) = module.lambdas.iter().next().expect("lambda entity");
    let signature = &module.function_types[lambda.function_type];
    let invoke = &module.functions[lambda.definition.source_function()];
    assert_eq!(
        signature.parameter_types,
        vec![invoke.params[1].ty],
        "one composite source pattern is one complete function-type parameter"
    );
    assert_eq!(
        hir::type_name(&module, signature.parameter_types[0]),
        "Pair"
    );
    assert_eq!(
        invoke
            .params
            .iter()
            .map(|param| param.name.as_str())
            .collect::<Vec<_>>(),
        vec!["$closure", "$arg.0"],
        "the invoke ABI has one hidden closure receiver and one complete source parameter"
    );
    assert_eq!(invoke.params[0].ty, signature.canonical_type);
    let hir::FunctionKind::User(body) = &invoke.kind else {
        panic!("lambda invoke body")
    };
    let source = invoke.params[1].local;
    assert_eq!(body.locals[source].name, "$arg.0");
    assert_eq!(body.locals[source].ty, signature.parameter_types[0]);
    assert!(!body.locals[source].mutable);
    assert!(
        body.statements.iter().any(|statement| matches!(
            &statement.kind,
            hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { .. },
                init: hir::Expr {
                    kind: hir::ExprKind::Local(local),
                    ..
                },
            } if *local == source
        )),
        "component setup must read the existing ABI parameter as its subject"
    );
    let calls = body
        .statements
        .iter()
        .filter_map(|statement| {
            let hir::StatementKind::ValDecl { init, .. } = &statement.kind else {
                return None;
            };
            matches!(init.kind, hir::ExprKind::MethodCall { .. })
                .then(|| direct_method_name(&module, init))
        })
        .collect::<Vec<_>>();
    assert_eq!(calls, vec!["Pair.component1", "Pair.component2"]);
    assert!(
        body.statements[..body.statements.len() - 1]
            .iter()
            .all(|statement| matches!(
                statement.kind,
                hir::StatementKind::ValDecl {
                    pattern: hir::Pattern::Binding { .. },
                    ..
                }
            )),
        "the complete plan prefix must expand before the source-body return"
    );
    assert!(
        !body
            .locals
            .iter()
            .any(|(_, local)| local.name.starts_with("$binding.subject."))
    );
}

#[test]
fn failed_composite_lambda_owner_rolls_back_earlier_parameters_and_function_counter() {
    let first_span = Span::new(410, 415);
    let missing_span = Span::new(420, 427);
    let suspend_span = Span::new(430, 438);
    let escaped_span = Span::new(440, 445);
    let failed_header = ast::Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters: Some(vec![
            ast::LambdaParam {
                target: pat_tuple(vec![pat_bind_at("first", first_span)], None),
                ty: None,
                span: first_span,
            },
            ast::LambdaParam {
                target: pat_tuple(vec![pat_bind_at("missing", missing_span)], None),
                ty: None,
                span: missing_span,
            },
        ]),
        body: block(vec![stmt(int_lit(0))]),
        span: sp(),
    };
    let failed_effect = ast::Expr::Lambda {
        id: ast::LambdaId(1),
        is_suspend: false,
        parameters: Some(vec![ast::LambdaParam {
            target: pat_tuple(vec![pat_bind_at("value", suspend_span)], None),
            ty: None,
            span: suspend_span,
        }]),
        body: block(vec![stmt(var("value"))]),
        span: sp(),
    };
    let escaped = ast::Expr::Var(ident_at("first", escaped_span));
    let errors = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Complete",
            vec![],
            None,
            vec![],
            vec![operator(method_expr(
                "component1",
                vec![],
                Some(ty_named("Int")),
                int_lit(1),
            ))],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "Incomplete",
            vec![],
            None,
            vec![],
            vec![],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "AsyncRollback",
            vec![],
            None,
            vec![],
            vec![operator(with_suspend(method_expr(
                "component1",
                vec![],
                Some(ty_named("Int")),
                int_lit(1),
            )))],
        ),
        fun(
            "main",
            vec![
                val_ty(
                    "failedHeader",
                    Some(ty_function(
                        false,
                        vec![ty_named("Complete"), ty_named("Incomplete")],
                        ty_named("Int"),
                    )),
                    failed_header,
                ),
                val_ty(
                    "failedEffect",
                    Some(ty_function(
                        false,
                        vec![ty_named("AsyncRollback")],
                        ty_named("Int"),
                    )),
                    failed_effect,
                ),
                stmt(escaped),
            ],
        ),
    ]))
    .expect_err("a failed lambda owner must not commit any partial lowering state");

    assert_eq!(errors.len(), 3, "{errors:?}");
    assert_eq!(
        errors[0].message,
        "type `Incomplete` has no method `component1`"
    );
    assert_eq!(errors[0].span, Some(missing_span));
    assert_eq!(
        errors[1].message,
        "suspend function `AsyncRollback.component1` cannot be called from non-suspend function `$lambda.0`",
        "the failed first lambda must roll back its generated-function counter"
    );
    assert_eq!(errors[1].span, Some(suspend_span));
    assert!(
        errors[2].message.starts_with("unknown variable `first`"),
        "the successfully planned first parameter of the failed owner must not escape"
    );
    assert_eq!(errors[2].span, Some(escaped_span));
}

#[test]
fn class_binding_plan_checks_suspend_components_in_the_lambda_context() {
    let async_class = || {
        class_decl(
            ast::ClassModifier::Final,
            "AsyncValue",
            vec![],
            None,
            vec![],
            vec![operator(with_suspend(method_expr(
                "component1",
                vec![],
                Some(ty_named("Int")),
                int_lit(1),
            )))],
        )
    };
    let operation = |is_suspend| ast::Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend,
        parameters: Some(vec![ast::LambdaParam {
            target: pat_tuple(vec![pat_bind("value")], None),
            ty: None,
            span: sp(),
        }]),
        body: block(vec![stmt(var("value"))]),
        span: sp(),
    };
    let errors = lower_user(file(vec![
        async_class(),
        fun(
            "main",
            vec![val_ty(
                "operation",
                Some(ty_function(
                    false,
                    vec![ty_named("AsyncValue")],
                    ty_named("Int"),
                )),
                operation(false),
            )],
        ),
    ]))
    .expect_err("an ordinary lambda cannot invoke a suspend component");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].message.contains(
        "suspend function `AsyncValue.component1` cannot be called from non-suspend function `$lambda.0`"
    ));

    let module = lower_user(file(vec![
        async_class(),
        fun(
            "main",
            vec![val_ty(
                "operation",
                Some(ty_function(
                    true,
                    vec![ty_named("AsyncValue")],
                    ty_named("Int"),
                )),
                operation(true),
            )],
        ),
    ]))
    .expect("a suspend lambda may invoke a suspend component");
    let (_, lambda) = module.lambdas.iter().next().expect("lambda entity");
    assert!(module.functions[lambda.definition.source_function()].is_suspend);
    let hir::FunctionKind::User(body) = &module.functions[lambda.definition.source_function()].kind
    else {
        panic!("lambda invoke body")
    };
    assert!(body.statements.iter().any(|statement| matches!(
        &statement.kind,
        hir::StatementKind::ValDecl {
            init: hir::Expr {
                kind: hir::ExprKind::MethodCall { .. },
                ..
            },
            ..
        }
    )));
}

#[test]
fn multi_index_get_and_set_use_typed_roles_and_set_reserves_its_value() {
    let get = operator(method_expr(
        "get",
        vec![("row", ty_named("Int")), ("column", ty_named("Int"))],
        Some(ty_named("Int")),
        var("row"),
    ));
    let mut set = operator(method_expr(
        "set",
        vec![("indices", ty_named("Int")), ("value", ty_named("Int"))],
        None,
        unit_lit(),
    ));
    set.params[0].syntax = ast::ParameterSyntax::Vararg {
        modifier_span: sp(),
        default: ast::VarargDefaultSyntax::EmptyWhenOmitted,
    };
    let module = lower_user(file(vec![
        struct_decl_methods("Grid", vec![], vec![get, set]),
        fun(
            "main",
            vec![
                val("grid", call("Grid", vec![])),
                val(
                    "read",
                    multi_index(var("grid"), vec![int_lit(1), int_lit(2)]),
                ),
                assignment(
                    index_place(var("grid"), vec![int_lit(3), int_lit(4)]),
                    ast::AssignmentOp::Assign,
                    int_lit(5),
                ),
            ],
        ),
    ]))
    .expect("multi-index get/set must use ordinary typed operator calls");
    let body = function_body(&module, "main");
    assert_eq!(
        direct_method_name(&module, local_init(body, "read")),
        "Grid.get"
    );
    let set_call = body
        .statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            hir::StatementKind::Expr(expr) => Some(expr),
            _ => None,
        })
        .find(|expr| direct_method_name(&module, expr) == "Grid.set")
        .expect("set call statement");
    let hir::ExprKind::MethodCall { args, .. } = &set_call.kind else {
        unreachable!("direct_method_name already checked the method shape")
    };
    assert_eq!(args.len(), 2, "vararg indices plus the reserved value");
    assert!(matches!(args[0].kind, hir::ExprKind::Local(_)));
    assert_eq!(args[1].ty, int_type(&module));
}

#[test]
fn update_and_compound_assignment_normalize_to_calls_and_writes() {
    let inc = operator(method_expr(
        "inc",
        vec![],
        Some(ty_named("Counter")),
        this_expr(),
    ));
    let plus = operator(method_expr(
        "plus",
        vec![("other", ty_named("Counter"))],
        Some(ty_named("Counter")),
        this_expr(),
    ));
    let plus_assign = operator(method_expr(
        "plusAssign",
        vec![("other", ty_named("Accumulator"))],
        None,
        unit_lit(),
    ));
    let module = lower_user(file(vec![
        struct_decl_methods("Counter", vec![], vec![inc, plus]),
        struct_decl_methods("Accumulator", vec![], vec![plus_assign]),
        fun(
            "main",
            vec![
                var_("counter", call("Counter", vec![])),
                val(
                    "postfix",
                    update(
                        ast::PlaceExpr::Name(ident("counter")),
                        ast::UpdateOp::Increment,
                        ast::UpdateNotation::Postfix,
                    ),
                ),
                val(
                    "prefix",
                    update(
                        ast::PlaceExpr::Name(ident("counter")),
                        ast::UpdateOp::Increment,
                        ast::UpdateNotation::Prefix,
                    ),
                ),
                assignment(
                    ast::PlaceExpr::Name(ident("counter")),
                    ast::AssignmentOp::Compound(ast::CompoundAssignOp::Add),
                    call("Counter", vec![]),
                ),
                val("accumulator", call("Accumulator", vec![])),
                assignment(
                    ast::PlaceExpr::Name(ident("accumulator")),
                    ast::AssignmentOp::Compound(ast::CompoundAssignOp::Add),
                    call("Accumulator", vec![]),
                ),
            ],
        ),
    ]))
    .expect("update, fallback and opAssign-only paths must all lower");
    let body = function_body(&module, "main");
    let dump = hir::dump(&module);
    assert_eq!(dump.matches("MethodCall Counter.inc").count(), 2, "{dump}");
    assert_eq!(dump.matches("MethodCall Counter.plus").count(), 1, "{dump}");
    assert_eq!(
        dump.matches("MethodCall Accumulator.plusAssign").count(),
        1,
        "{dump}"
    );
    assert_eq!(
        body.statements
            .iter()
            .filter(|statement| matches!(
                statement.kind,
                hir::StatementKind::Assign {
                    target: hir::AssignTarget::Local(local),
                    ..
                } if body.locals[local].name == "counter"
            ))
            .count(),
        3
    );
    assert!(matches!(
        local_init(body, "postfix").kind,
        hir::ExprKind::Local(_)
    ));
    assert!(matches!(
        local_init(body, "prefix").kind,
        hir::ExprKind::Local(_)
    ));
}

#[test]
fn compound_assignment_probes_both_roles_and_reuses_index_sources() {
    let plus = operator(method_expr(
        "plus",
        vec![("other", ty_named("Number"))],
        Some(ty_named("Number")),
        this_expr(),
    ));
    let plus_assign = operator(method_expr(
        "plusAssign",
        vec![("other", ty_named("Number"))],
        None,
        unit_lit(),
    ));
    let get = operator(method_expr(
        "get",
        vec![("index", ty_named("Int"))],
        Some(ty_named("Number")),
        call("Number", vec![]),
    ));
    let set = operator(method_expr(
        "set",
        vec![("index", ty_named("Int")), ("value", ty_named("Number"))],
        None,
        unit_lit(),
    ));
    let module = lower_user(file(vec![
        struct_decl_methods("Number", vec![], vec![plus.clone()]),
        struct_decl_methods("Table", vec![], vec![get.clone(), set]),
        fun_expr(
            "makeTable",
            vec![],
            vec![],
            Some(ty_named("Table")),
            call("Table", vec![]),
        ),
        fun_expr(
            "nextIndex",
            vec![],
            vec![],
            Some(ty_named("Int")),
            int_lit(0),
        ),
        fun_expr(
            "rhs",
            vec![],
            vec![],
            Some(ty_named("Number")),
            call("Number", vec![]),
        ),
        fun(
            "main",
            vec![assignment(
                index_place(call("makeTable", vec![]), vec![call("nextIndex", vec![])]),
                ast::AssignmentOp::Compound(ast::CompoundAssignOp::Add),
                call("rhs", vec![]),
            )],
        ),
    ]))
    .expect("indexed fallback must share receiver and source-index temporaries");
    let dump = hir::dump(&module);
    for call_name in ["makeTable", "nextIndex", "rhs"] {
        assert_eq!(
            dump.matches(&format!("Call {call_name}")).count(),
            1,
            "{call_name} must execute once:\n{dump}"
        );
    }
    assert_eq!(dump.matches("MethodCall Table.get").count(), 1, "{dump}");
    assert_eq!(dump.matches("MethodCall Table.set").count(), 1, "{dump}");

    let errors = lower_user(file(vec![
        struct_decl_methods("Number", vec![], vec![plus, plus_assign]),
        fun(
            "main",
            vec![
                var_("number", call("Number", vec![])),
                assignment(
                    ast::PlaceExpr::Name(ident("number")),
                    ast::AssignmentOp::Compound(ast::CompoundAssignOp::Add),
                    call("Number", vec![]),
                ),
            ],
        ),
    ]))
    .expect_err("simultaneously applicable operator groups must be ambiguous");
    assert!(errors.iter().any(|error| {
        error.message
            == "compound assignment is ambiguous: both `plusAssign` and `plus` are applicable"
    }));
}

#[test]
fn safe_methods_keep_calls_defaults_and_property_invoke_inside_some_branch() {
    let invoke = operator(method_expr(
        "invoke",
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        var("value"),
    ));
    let maybe = method_expr(
        "maybe",
        vec![],
        Some(ty_nullable(ty_named("Int"))),
        some(int_lit(1)),
    );
    let ping = method_expr("ping", vec![], None, unit_lit());
    let mut send = method_expr(
        "send",
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        var("value"),
    );
    send.params[0].syntax = ast::ParameterSyntax::Default {
        expression: call("defaultValue", vec![]),
        equals_span: sp(),
    };
    let module = lower_user(file(vec![
        struct_decl_methods("Handler", vec![], vec![invoke]),
        class_decl(
            ast::ClassModifier::Final,
            "Client",
            vec![
                (false, "handler", ty_named("Handler")),
                (
                    false,
                    "callback",
                    ty_function(false, vec![ty_named("Int")], ty_named("Int")),
                ),
            ],
            None,
            vec![],
            vec![maybe, ping, send],
        ),
        fun_expr(
            "defaultValue",
            vec![],
            vec![],
            Some(ty_named("Int")),
            int_lit(2),
        ),
        fun(
            "main",
            vec![
                val_ty(
                    "client",
                    Some(ty_nullable(ty_named("Client"))),
                    some(call(
                        "Client",
                        vec![call("Handler", vec![]), int_identity_lambda()],
                    )),
                ),
                val("nested", safe_method_call(var("client"), "maybe", vec![])),
                val("unit", safe_method_call(var("client"), "ping", vec![])),
                val("defaulted", safe_method_call(var("client"), "send", vec![])),
                val(
                    "property",
                    safe_method_call(var("client"), "handler", vec![int_lit(3)]),
                ),
                val(
                    "function_property",
                    safe_method_call(var("client"), "callback", vec![int_lit(4)]),
                ),
            ],
        ),
    ]))
    .expect("safe calls must reuse ordinary method/property-like resolution in the Some branch");
    let body = function_body(&module, "main");
    let dump = hir::dump(&module);
    assert!(dump.contains("SomeWrap : Option<Option<Int>>"), "{dump}");
    assert!(dump.contains("SomeWrap : Option<Unit>"), "{dump}");
    assert!(dump.contains("MethodCall Handler.invoke"), "{dump}");
    assert!(dump.contains("CallableCall function_type"), "{dump}");
    assert_eq!(dump.matches("Call defaultValue").count(), 1, "{dump}");
    assert_eq!(
        body.statements
            .iter()
            .filter(|statement| matches!(statement.kind, hir::StatementKind::If { .. }))
            .count(),
        5
    );
    assert!(matches!(
        local_init(body, "nested").kind,
        hir::ExprKind::Local(_)
    ));
    assert!(matches!(
        local_init(body, "property").kind,
        hir::ExprKind::Local(_)
    ));
}

#[test]
fn safe_suspend_call_keeps_the_call_in_the_suspend_branch() {
    let fetch = with_suspend(method_expr(
        "fetch",
        vec![],
        Some(ty_named("Int")),
        int_lit(1),
    ));
    let Decl::Function(mut run) = fun(
        "run",
        vec![
            val_ty(
                "worker",
                Some(ty_nullable(ty_named("Worker"))),
                some(call("Worker", vec![])),
            ),
            val("result", safe_method_call(var("worker"), "fetch", vec![])),
        ],
    ) else {
        unreachable!("fun builds a function")
    };
    run.is_suspend = true;
    let module = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Worker",
            vec![],
            None,
            vec![],
            vec![fetch],
        ),
        Decl::Function(run),
        fun("main", vec![]),
    ]))
    .expect("a safe suspend call is valid in a suspend body");
    let dump = hir::dump(&module);
    assert!(dump.contains("suspend fun run()"), "{dump}");
    assert!(dump.contains("MethodCall Worker.fetch"), "{dump}");
    let body = function_body(&module, "run");
    assert!(
        body.statements
            .iter()
            .any(|statement| matches!(statement.kind, hir::StatementKind::If { .. }))
    );
}

#[test]
fn safe_extension_in_while_stays_in_the_repeated_condition_setup() {
    let extension = extension_expr(
        ty_named("Client"),
        "ready",
        Vec::new(),
        vec![],
        Some(ty_named("Boolean")),
        bool_lit(true),
    );
    let module = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Client",
            vec![],
            None,
            vec![],
            vec![],
        ),
        extension,
        fun(
            "main",
            vec![
                val_ty(
                    "client",
                    Some(ty_nullable(ty_named("Client"))),
                    some(call("Client", vec![])),
                ),
                while_stmt(
                    elvis(
                        safe_method_call(var("client"), "ready", vec![]),
                        bool_lit(false),
                    ),
                    vec![],
                ),
            ],
        ),
    ]))
    .expect("safe extension setup must be evaluated on every loop condition");
    let body = function_body(&module, "main");
    let hir::StatementKind::While {
        condition_setup,
        cond,
        ..
    } = &body
        .statements
        .iter()
        .find(|statement| matches!(statement.kind, hir::StatementKind::While { .. }))
        .expect("while statement")
        .kind
    else {
        unreachable!("the matching statement is a while")
    };
    assert!(!condition_setup.is_empty());
    assert_eq!(cond.ty, module.boolean);
    assert!(hir::dump(&module).contains("Call ready"));

    let errors = lower_user(file(vec![fun(
        "main",
        vec![stmt(safe_method_call(int_lit(1), "ready", vec![]))],
    )]))
    .expect_err("safe navigation requires an exact Option receiver");
    assert!(
        errors
            .iter()
            .any(|error| { error.message == "`?.` requires an Option receiver, found Int" })
    );
}

#[test]
fn override_must_preserve_the_infix_contract() {
    let declaration = infix(method_full(
        false,
        true,
        "merge",
        vec![("other", ty_named("Joinable"))],
        Some(ty_named("Joinable")),
        FunctionBody::None,
    ));
    let implementation = method_full(
        true,
        false,
        "merge",
        vec![("other", ty_named("Joinable"))],
        Some(ty_named("Joinable")),
        FunctionBody::Expr(Box::new(var("other"))),
    );
    let errors = lower_user(file(vec![
        interface_decl("Joinable", vec![declaration]),
        class_decl(
            ast::ClassModifier::Final,
            "Joiner",
            vec![],
            None,
            vec!["Joinable"],
            vec![implementation],
        ),
        fun("main", vec![]),
    ]))
    .expect_err("override signatures include their infix identity");
    assert!(errors.iter().any(|error| {
        error.message == "`merge` must have the same `infix` modifier as `Joinable.merge`"
    }));
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

#[test]
fn class_destructuring_uses_a_typed_extension_component_action() {
    let component = operator_extension(
        ty_named("ExtensionParts"),
        "component1",
        vec![],
        ty_named("Int"),
        int_lit(42),
    );
    let module = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "ExtensionParts",
            vec![],
            None,
            vec![],
            vec![],
        ),
        component,
        fun(
            "main",
            vec![val_pat(
                false,
                pat_tuple(vec![pat_bind("value")], None),
                None,
                call("ExtensionParts", vec![]),
            )],
        ),
    ]))
    .expect("a typed extension component enables class destructuring");
    let body = function_body(&module, "main");

    let subjects = body
        .statements
        .iter()
        .filter_map(|statement| {
            let hir::StatementKind::ValDecl { pattern, init } = &statement.kind else {
                return None;
            };
            let local = binding_local(pattern)?;
            body.locals[local]
                .name
                .starts_with("$binding.subject.")
                .then_some((local, init))
        })
        .collect::<Vec<_>>();
    assert_eq!(subjects.len(), 1, "the class subject is materialized once");
    assert!(matches!(
        subjects[0].1.kind,
        hir::ExprKind::ClassInit { .. }
    ));
    assert!(!body.locals[subjects[0].0].mutable);

    let component_calls = body
        .statements
        .iter()
        .filter_map(|statement| {
            let hir::StatementKind::ValDecl { pattern, init } = &statement.kind else {
                return None;
            };
            let local = binding_local(pattern)?;
            let hir::ExprKind::Call {
                callee: hir::CallableTarget::Local(callee),
                ..
            } = &init.kind
            else {
                return None;
            };
            let function = module.callable_function(*callee);
            (module.functions[function].modifiers.operator
                == Some(hir::OperatorKind::Component {
                    index: std::num::NonZeroU32::new(1).expect("one is nonzero"),
                }))
            .then_some((local, init, function))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        component_calls.len(),
        1,
        "exactly one typed extension component is called"
    );
    let (component_local, component_call, component_function) = component_calls[0];
    assert_eq!(module.functions[component_function].name, "component1");
    assert_eq!(direct_callable_name(&module, component_call), "component1");
    assert!(
        body.locals[component_local]
            .name
            .starts_with("$binding.component.")
    );
    assert!(!body.locals[component_local].mutable);
    assert_eq!(body.locals[component_local].ty, int_type(&module));

    let hir::ExprKind::Local(bound_from) = &local_init(body, "value").kind else {
        panic!("the user leaf must read the completed extension component temporary")
    };
    assert_eq!(*bound_from, component_local);
}

fn assert_adapted_extension_component(
    module: &hir::Module,
    subject_type: &str,
    receiver_type: &str,
) {
    let body = function_body(module, "main");
    let subjects = body
        .statements
        .iter()
        .filter_map(|statement| {
            let hir::StatementKind::ValDecl { pattern, init } = &statement.kind else {
                return None;
            };
            let local = binding_local(pattern)?;
            body.locals[local]
                .name
                .starts_with("$binding.subject.")
                .then_some((local, init))
        })
        .collect::<Vec<_>>();
    let [(subject, subject_init)] = subjects.as_slice() else {
        panic!("class destructuring must materialize exactly one subject")
    };
    assert_eq!(
        hir::type_name(module, body.locals[*subject].ty),
        subject_type
    );
    assert_eq!(subject_init.ty, body.locals[*subject].ty);

    let receivers = body
        .statements
        .iter()
        .filter_map(|statement| {
            let hir::StatementKind::ValDecl { pattern, init } = &statement.kind else {
                return None;
            };
            let local = binding_local(pattern)?;
            body.locals[local]
                .name
                .eq("$receiver")
                .then_some((local, init))
        })
        .collect::<Vec<_>>();
    let [(receiver, receiver_init)] = receivers.as_slice() else {
        panic!("one extension component must materialize exactly one receiver")
    };
    let hir::ExprKind::ReferenceUpcast(source_expression) = &receiver_init.kind else {
        panic!("the component receiver retains its reference adaptation")
    };
    let hir::ExprKind::Local(receiver_source) = source_expression.kind else {
        panic!("the adapted extension receiver must read the class subject")
    };
    assert_eq!(receiver_source, *subject);
    assert_eq!(source_expression.ty, body.locals[*subject].ty);
    assert_eq!(hir::type_name(module, receiver_init.ty), receiver_type);
    assert_eq!(body.locals[*receiver].ty, receiver_init.ty);
    assert!(!body.locals[*receiver].mutable);

    let components = body
        .statements
        .iter()
        .filter_map(|statement| {
            let hir::StatementKind::ValDecl { pattern, init } = &statement.kind else {
                return None;
            };
            let local = binding_local(pattern)?;
            body.locals[local]
                .name
                .starts_with("$binding.component.")
                .then_some((local, init))
        })
        .collect::<Vec<_>>();
    let [(component, call)] = components.as_slice() else {
        panic!("one written position must have exactly one component result")
    };
    let hir::ExprKind::Call {
        callee: hir::CallableTarget::Local(callee),
        args,
        ..
    } = &call.kind
    else {
        panic!("an extension component must remain a direct typed call")
    };
    let [argument] = args.as_slice() else {
        panic!("an extension component has exactly its receiver argument")
    };
    let hir::ExprKind::Local(argument_source) = argument.kind else {
        panic!("the component call must read its materialized receiver")
    };
    assert_eq!(argument_source, *receiver);
    assert_eq!(argument.ty, receiver_init.ty);

    let winner = module.callable_function(*callee);
    assert_eq!(module.functions[winner].name, "component1");
    assert_eq!(module.functions[winner].method, None);
    assert_eq!(
        module.functions[winner].modifiers.operator,
        Some(hir::OperatorKind::Component {
            index: std::num::NonZeroU32::new(1).expect("one is nonzero"),
        }),
        "the adapted call must retain the exact typed component1 winner"
    );
    assert_eq!(call.ty, int_type(module));
    assert_eq!(body.locals[*component].ty, call.ty);
    assert!(!body.locals[*component].mutable);

    let hir::ExprKind::Local(bound_from) = local_init(body, "value").kind else {
        panic!("the user leaf must read the completed component temporary")
    };
    assert_eq!(bound_from, *component);
}

#[test]
fn class_destructuring_adapts_a_base_extension_component_receiver() {
    let module = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Open,
            "ComponentBase",
            vec![],
            None,
            vec![],
            vec![],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "DerivedParts",
            vec![],
            Some(("ComponentBase", vec![])),
            vec![],
            vec![],
        ),
        operator_extension(
            ty_named("ComponentBase"),
            "component1",
            vec![],
            ty_named("Int"),
            int_lit(41),
        ),
        fun(
            "main",
            vec![val_pat(
                false,
                pat_tuple(vec![pat_bind("value")], None),
                None,
                call("DerivedParts", vec![]),
            )],
        ),
    ]))
    .expect("a class may use a component extension declared on its base type");

    assert_adapted_extension_component(&module, "DerivedParts", "ComponentBase");
}

#[test]
fn class_destructuring_adapts_an_interface_extension_component_receiver() {
    let module = lower_user(file(vec![
        interface_decl("ComponentView", vec![]),
        class_decl(
            ast::ClassModifier::Final,
            "ViewedParts",
            vec![],
            None,
            vec!["ComponentView"],
            vec![],
        ),
        operator_extension(
            ty_named("ComponentView"),
            "component1",
            vec![],
            ty_named("Int"),
            int_lit(42),
        ),
        fun(
            "main",
            vec![val_pat(
                false,
                pat_tuple(vec![pat_bind("value")], None),
                None,
                call("ViewedParts", vec![]),
            )],
        ),
    ]))
    .expect("a class may use a component extension declared on an implemented interface");

    assert_adapted_extension_component(&module, "ViewedParts", "ComponentView");
}
