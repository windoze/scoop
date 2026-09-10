use super::*;

#[test]
fn operator_equals_is_a_typed_hir_contract() {
    let output = lower_user_output(file(vec![
        interface_decl(
            "EqualTo",
            vec![operator_equals(false, true, ty_named("EqualTo"))],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "Value",
            Vec::new(),
            None,
            vec!["EqualTo"],
            vec![operator_equals(true, false, ty_named("EqualTo"))],
        ),
        fun(
            "main",
            vec![
                val("left", call("Value", Vec::new())),
                val("right", call("Value", Vec::new())),
                val("same", binary(BinOp::Eq, var("left"), var("right"))),
                val("different", binary(BinOp::Ne, var("left"), var("right"))),
            ],
        ),
    ]))
    .expect("operator identity must participate in ordinary interface conformance");

    let methods = output
        .export
        .functions
        .iter()
        .filter(|(_, function)| matches!(function.name.as_str(), "EqualTo.equals" | "Value.equals"))
        .map(|(_, function)| function.modifiers.operator)
        .collect::<Vec<_>>();
    assert_eq!(
        methods,
        vec![
            Some(hir::OperatorKind::Equals),
            Some(hir::OperatorKind::Equals)
        ]
    );
    assert!(hir::dump(&output.export).contains("operator fun equals(other: EqualTo): Boolean"));
    assert!(output.local.functions.iter().any(|(_, function)| {
        function.name == "Value.equals"
            && function.modifiers.operator == Some(hir::OperatorKind::Equals)
    }));
    let dump = hir::dump(&output.export);
    assert!(dump.contains("MethodCall Value.equals : Boolean"), "{dump}");
    assert!(
        dump.contains("Unary Not : Boolean\n        MethodCall Value.equals : Boolean"),
        "{dump}"
    );

    let plain_equals = method_full(
        true,
        false,
        "equals",
        vec![("other", ty_named("EqualTo"))],
        Some(ty_named("Boolean")),
        FunctionBody::Expr(Box::new(bool_lit(true))),
    );
    let errors = lower_user(file(vec![
        interface_decl(
            "EqualTo",
            vec![operator_equals(false, true, ty_named("EqualTo"))],
        ),
        class_decl(
            ast::ClassModifier::Final,
            "Plain",
            Vec::new(),
            None,
            vec!["EqualTo"],
            vec![plain_equals],
        ),
        fun("main", Vec::new()),
    ]))
    .expect_err("the operator bit is part of the interface method contract");
    assert!(errors.iter().any(|error| {
        error.message == "`equals` must have the same `operator` modifier as `EqualTo.equals`"
    }));
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("does not implement interface method `EqualTo.equals`")
    }));
}

#[test]
fn generic_equality_resolves_the_exact_operator_bound_member() {
    let equality = generic_interface_decl(
        "Equality",
        vec!["T"],
        vec![operator_equals(false, true, ty_named("T"))],
    );
    let mut value = struct_decl_full(
        "Value",
        Vec::new(),
        Vec::new(),
        vec![operator_equals(true, false, ty_named("Value"))],
    );
    let Decl::Struct(value_decl) = &mut value else {
        unreachable!()
    };
    value_decl.supertypes = vec![bare_supertype(ty_generic(
        "Equality",
        vec![ty_named("Value")],
    ))];

    let mut equal = fun_expr(
        "equal",
        vec!["T"],
        vec![("left", ty_named("T")), ("right", ty_named("T"))],
        Some(ty_named("Boolean")),
        binary(BinOp::Eq, var("left"), var("right")),
    );
    let Decl::Function(equal_decl) = &mut equal else {
        unreachable!()
    };
    equal_decl.type_params[0] = upper("T", ty_generic("Equality", vec![ty_named("T")]));

    let output = lower_user_output(file(vec![
        equality,
        value,
        equal,
        fun(
            "main",
            vec![stmt(call(
                "println",
                vec![call(
                    "equal",
                    vec![
                        struct_init("Value", Vec::new()),
                        struct_init("Value", Vec::new()),
                    ],
                )],
            ))],
        ),
    ]))
    .expect("the F-bound exposes its exact operator member");

    let dump = hir::dump(&output.export);
    assert!(
        dump.contains("MethodCall bound T0 via Equality<T0> -> Equality.equals : Boolean"),
        "{dump}"
    );
    let concrete = output
        .local
        .functions
        .iter()
        .find(|(_, function)| {
            function.name == "equal"
                && matches!(
                    &function.emission,
                    hir::concrete::FunctionEmission::Materialized { .. }
                )
        })
        .expect("equal<Value> specialization")
        .1;
    let hir::concrete::FunctionKind::User(body) = &concrete.kind else {
        panic!("equal<Value> has a concrete body")
    };
    let hir::concrete::StatementKind::Return {
        value:
            Some(hir::concrete::Expr {
                kind: hir::concrete::ExprKind::MethodCall { callee, .. },
                ..
            }),
    } = &body.statements[0].kind
    else {
        panic!("the concrete body returns the resolved operator call")
    };
    let target = output.local.callable_function(*callee);
    assert_eq!(output.local.functions[target].name, "Value.equals");
}

#[test]
fn derived_equality_normalizes_integer_field_winners_before_concretization() {
    let output = lower_user_output(file(vec![
        generic_struct_decl(
            "Pair",
            vec!["T"],
            vec![("first", ty_named("T")), ("second", ty_named("T"))],
        ),
        fun(
            "main",
            vec![val(
                "same",
                binary(
                    BinOp::Eq,
                    struct_init("Pair", vec![int_lit(1), int_lit(2)]),
                    struct_init("Pair", vec![int_lit(1), int_lit(2)]),
                ),
            )],
        ),
    ]))
    .expect("derived equality over integer fields must remain closed HIR");

    let application = output
        .export
        .derived_equality_applications
        .iter()
        .find_map(|(_, application)| {
            (hir::type_name(&output.export, application.owner_ty) == "Pair<Int>")
                .then_some(application)
        })
        .expect("Pair<Int> derived equality application");
    let hir::ExprKind::Binary {
        op: hir::BinOp::And,
        lhs,
        rhs,
    } = &return_value(&application.body.statements).kind
    else {
        panic!("Pair<Int> equality compares both fields")
    };
    for comparison in [lhs.as_ref(), rhs.as_ref()] {
        assert!(matches!(
            comparison.kind,
            hir::ExprKind::IntegerOperation {
                operation: hir::IntegerOperation::NoGc {
                    kind: hir::IntegerKind::SIGNED_32,
                    operation: hir::NoGcIntegerOperation::Equals,
                    ..
                },
                ..
            }
        ));
    }

    let concrete = output
        .local
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == "Pair.equals").then_some(function))
        .expect("concrete Pair<Int> derived equality function");
    let hir::concrete::FunctionKind::User(body) = &concrete.kind else {
        panic!("derived equality has a concrete user body")
    };
    let value = body
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::concrete::StatementKind::Return { value: Some(value) } => Some(value),
            _ => None,
        })
        .expect("derived equality returns its comparison");
    let hir::concrete::ExprKind::Binary {
        op: hir::BinOp::And,
        lhs,
        rhs,
    } = &value.kind
    else {
        panic!("concrete Pair<Int> equality compares both fields")
    };
    for comparison in [lhs.as_ref(), rhs.as_ref()] {
        assert!(matches!(
            comparison.kind,
            hir::concrete::ExprKind::IntegerOperation {
                operation: hir::IntegerOperation::NoGc {
                    kind: hir::IntegerKind::SIGNED_32,
                    operation: hir::NoGcIntegerOperation::Equals,
                    ..
                },
                ..
            }
        ));
    }
}

#[test]
fn operator_equals_legality_is_checked_at_its_declaration() {
    let mut wrong_name = operator_equals(false, false, ty_named("Bad"));
    wrong_name.name = ident("compare");
    let mut wrong_arity = operator_equals(false, false, ty_named("Bad"));
    wrong_arity.params.push(ast::Param {
        name: ident("extra"),
        ty: ty_named("Bad"),
        syntax: ast::ParameterSyntax::Required,
        span: sp(),
    });
    let mut wrong_result = operator_equals(false, false, ty_named("Bad"));
    wrong_result.return_ty = Some(ty_named("Int"));
    let mut generic = operator_equals(false, false, ty_named("T"));
    generic.type_params = vec![type_param("T")];
    let mut suspended = operator_equals(false, false, ty_named("Bad"));
    suspended.is_suspend = true;

    let errors = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Bad",
            Vec::new(),
            None,
            Vec::new(),
            vec![wrong_name, wrong_arity, wrong_result, generic, suspended],
        ),
        fun("main", Vec::new()),
    ]))
    .expect_err("every invalid operator shape must be rejected before HIR output");
    for expected in [
        "unknown operator role `compare`",
        "operator `equals` must have exactly one parameter, found 2",
        "operator `equals` must return Boolean, found Int",
        "operator `equals` must not declare type parameters",
        "operator `equals` must not be suspend",
    ] {
        assert!(
            errors.iter().any(|error| error.message == expected),
            "missing diagnostic: {expected}; found {errors:?}"
        );
    }
}

#[test]
fn operator_modifier_requires_a_dispatch_or_extension_receiver() {
    let mut top = fun("top", Vec::new());
    let Decl::Function(function) = &mut top else {
        unreachable!()
    };
    function.operator = Some(ast::OperatorModifier { span: sp() });
    let errors = lower_user(file(vec![top, fun("main", Vec::new())]))
        .expect_err("a free operator function has no receiver capability");
    assert!(
        errors
            .iter()
            .any(|error| error.message == "`operator` requires a member or extension receiver")
    );
}
