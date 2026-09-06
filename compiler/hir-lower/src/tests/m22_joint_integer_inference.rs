use super::*;

fn large() -> Expr {
    int_lit(i64::from(i32::MAX) + 1)
}

fn infix_integer(lhs: Expr, name: &str, rhs: Expr) -> Expr {
    Expr::InfixCall {
        lhs: Box::new(lhs),
        target: ast::InfixTarget::Named(ident(name)),
        rhs: Box::new(rhs),
        span: sp(),
    }
}

fn if_expr(then_value: Expr, else_value: Expr) -> Expr {
    Expr::If(Box::new(ast::If {
        cond: bool_lit(false),
        then_block: block(vec![stmt(then_value)]),
        else_block: Some(block(vec![stmt(else_value)])),
        span: sp(),
    }))
}

fn when_expr(first: Expr, second: Expr) -> Expr {
    Expr::When(Box::new(ast::When {
        subject: field(var("Choice"), "First"),
        arms: vec![
            arm(pat_bind("First"), None, vec![stmt(first)]),
            arm(pat_bind("Second"), None, vec![stmt(second)]),
        ],
        else_body: None,
        span: sp(),
    }))
}

fn try_expr(body: Expr, catch: Expr) -> Expr {
    Expr::Try(Box::new(ast::Try {
        body: block(vec![stmt(body)]),
        catches: vec![catch_clause(
            "error",
            ty_named("UnwrapException"),
            vec![stmt(catch)],
        )],
        finally_body: None,
        span: sp(),
    }))
}

fn main_body(module: &hir::Module) -> &hir::Body {
    let hir::FunctionKind::User(body) = &module.functions[module.entry].kind else {
        panic!("main must have a user body")
    };
    body
}

fn core_with_int8_ranges() -> SourceFile {
    let mut core = core_file();
    let int8 = core
        .declarations
        .iter_mut()
        .find_map(|declaration| match declaration {
            Decl::Struct(declaration) if declaration.name.text == "Int8" => Some(declaration),
            _ => None,
        })
        .expect("the test core declares Int8");
    for (name, operator, infix) in [
        ("rangeTo", true, false),
        ("rangeUntil", true, false),
        ("until", false, true),
        ("downTo", false, true),
    ] {
        let mut method = method_expr(
            name,
            vec![("other", ty_named("Int8"))],
            Some(ty_named("Int8")),
            this_expr(),
        );
        method.operator = operator.then_some(ast::OperatorModifier { span: sp() });
        method.infix = infix.then_some(ast::InfixModifier { span: sp() });
        int8.members
            .push(ast::StructMember::Function(Box::new(method)));
    }
    make_core_public(&mut core);
    core
}

#[test]
fn operators_and_explicit_members_join_literal_candidates_before_defaulting() {
    let module = lower_user(file(vec![fun(
        "main",
        vec![
            val("binaryForward", binary(BinOp::Add, int_lit(1), large())),
            val("binaryReverse", binary(BinOp::Add, large(), int_lit(1))),
            val("equalityForward", binary(BinOp::Eq, int_lit(1), large())),
            val("equalityReverse", binary(BinOp::Eq, large(), int_lit(1))),
            val(
                "memberForward",
                method_call(int_lit(1), "plus", vec![large()]),
            ),
            val(
                "memberReverse",
                method_call(large(), "plus", vec![int_lit(1)]),
            ),
            val("infixForward", infix_integer(int_lit(1), "and", large())),
            val("infixReverse", infix_integer(large(), "and", int_lit(1))),
            val_ty(
                "compareResultIsLong",
                Some(ty_named("Long")),
                method_call(int_lit(1), "compareTo", vec![int_lit(2)]),
            ),
        ],
    )]))
    .expect("joint literal constraints must be independent of operand order");
    let body = main_body(&module);
    let long = integer_type(&module, hir::IntegerKind::SIGNED_64);
    for name in [
        "binaryForward",
        "binaryReverse",
        "memberForward",
        "memberReverse",
        "infixForward",
        "infixReverse",
    ] {
        assert_eq!(local_init(body, name).ty, long, "{name}");
    }
    for name in ["equalityForward", "equalityReverse"] {
        assert_eq!(local_init(body, name).ty, module.boolean, "{name}");
    }
    let hir::ExprKind::IntegerOperation { operation, .. } =
        &local_init(body, "compareResultIsLong").kind
    else {
        panic!("compareTo must normalize to a typed integer operation")
    };
    assert_eq!(
        operation.kind(),
        hir::IntegerKind::SIGNED_32,
        "a Long result expectation must not be confused with the receiver kind"
    );
}

#[test]
fn generic_calls_constructors_and_arrays_share_the_strongest_literal_constraint() {
    let module = lower_user(file(vec![
        fun_expr(
            "same",
            vec!["T"],
            vec![("first", ty_named("T")), ("second", ty_named("T"))],
            Some(ty_named("T")),
            var("first"),
        ),
        generic_struct_decl(
            "Pair",
            vec!["T"],
            vec![("first", ty_named("T")), ("second", ty_named("T"))],
        ),
        fun(
            "main",
            vec![
                val("callForward", call("same", vec![int_lit(1), large()])),
                val("callReverse", call("same", vec![large(), int_lit(1)])),
                val(
                    "nestedCall",
                    call("same", vec![some(int_lit(1)), some(large())]),
                ),
                val(
                    "pairForward",
                    struct_init("Pair", vec![int_lit(1), large()]),
                ),
                val(
                    "pairReverse",
                    struct_init("Pair", vec![large(), int_lit(1)]),
                ),
                val(
                    "nestedPair",
                    struct_init("Pair", vec![some(int_lit(1)), some(large())]),
                ),
                val("arrayForward", array_lit(vec![int_lit(1), large()])),
                val("arrayReverse", array_lit(vec![large(), int_lit(1)])),
                val(
                    "nestedArray",
                    array_lit(vec![some(int_lit(1)), some(large())]),
                ),
            ],
        ),
    ]))
    .expect("generic and aggregate fixed points must join all literal evidence");
    let body = main_body(&module);
    for name in ["callForward", "callReverse"] {
        assert_eq!(hir::type_name(&module, local_init(body, name).ty), "Long");
    }
    for name in ["pairForward", "pairReverse"] {
        assert_eq!(
            hir::type_name(&module, local_init(body, name).ty),
            "Pair<Long>"
        );
    }
    assert_eq!(
        hir::type_name(&module, local_init(body, "nestedCall").ty),
        "Option<Long>"
    );
    assert_eq!(
        hir::type_name(&module, local_init(body, "nestedPair").ty),
        "Pair<Option<Long>>"
    );
    for name in ["arrayForward", "arrayReverse"] {
        assert_eq!(
            hir::type_name(&module, local_init(body, name).ty),
            "Array<Long>"
        );
    }
    assert_eq!(
        hir::type_name(&module, local_init(body, "nestedArray").ty),
        "Array<Option<Long>>"
    );
}

#[test]
fn control_value_fixed_points_join_nested_literals_independently_of_branch_order() {
    let module = lower_user(file(vec![
        enum_decl(
            "Choice",
            Vec::new(),
            vec![variant_unit("First"), variant_unit("Second")],
        ),
        fun(
            "main",
            vec![
                val("ifForward", if_expr(int_lit(1), large())),
                val("ifReverse", if_expr(large(), int_lit(1))),
                val("whenForward", when_expr(int_lit(1), large())),
                val("whenReverse", when_expr(large(), int_lit(1))),
                val("tryForward", try_expr(int_lit(1), large())),
                val("tryReverse", try_expr(large(), int_lit(1))),
                val("nestedIf", if_expr(some(int_lit(1)), some(large()))),
                val("nestedWhen", when_expr(some(int_lit(1)), some(large()))),
                val("nestedTry", try_expr(some(int_lit(1)), some(large()))),
            ],
        ),
    ]))
    .expect("branch fixed points must choose the strongest nested literal seed");
    let body = main_body(&module);
    for name in [
        "ifForward",
        "ifReverse",
        "whenForward",
        "whenReverse",
        "tryForward",
        "tryReverse",
    ] {
        assert_eq!(hir::type_name(&module, local_init(body, name).ty), "Long");
    }
    for name in ["nestedIf", "nestedWhen", "nestedTry"] {
        assert_eq!(
            hir::type_name(&module, local_init(body, name).ty),
            "Option<Long>"
        );
    }
}

#[test]
fn user_extension_spelling_cannot_unlock_non_default_literal_receivers() {
    let ordinary_default = extension_expr(
        ty_named("Int"),
        "until",
        Vec::new(),
        vec![("other", ty_named("Int"))],
        Some(ty_named("Int")),
        this_expr(),
    );
    let non_default = extension_expr(
        ty_named("Int8"),
        "until",
        Vec::new(),
        vec![("other", ty_named("Int8"))],
        Some(ty_named("Int8")),
        this_expr(),
    );
    let module = lower_user(file(vec![
        ordinary_default,
        non_default,
        fun(
            "main",
            vec![val(
                "ordinary",
                method_call(int_lit(1), "until", vec![int_lit(2)]),
            )],
        ),
    ]))
    .expect("ordinary literal defaulting may still select an Int extension");
    assert_eq!(
        hir::type_name(&module, local_init(main_body(&module), "ordinary").ty),
        "Int"
    );

    let errors = lower_user(file(vec![
        extension_expr(
            ty_named("Int8"),
            "until",
            Vec::new(),
            vec![("other", ty_named("Int8"))],
            Some(ty_named("Int8")),
            this_expr(),
        ),
        fun(
            "main",
            vec![
                val_ty("small", Some(ty_named("Int8")), int_lit(2)),
                val(
                    "invalid",
                    method_call(int_lit(1), "until", vec![var("small")]),
                ),
            ],
        ),
    ]))
    .expect_err("an extension must not reverse-infer a non-default literal receiver");
    assert!(
        errors.iter().any(|diagnostic| diagnostic
            .message
            .contains("no applicable candidate for `until`")),
        "{errors:#?}"
    );
}

#[test]
fn all_four_typed_core_range_roles_may_reverse_infer_a_literal_receiver() {
    let user = file(vec![fun(
        "main",
        vec![
            val_ty("endpoint", Some(ty_named("Int8")), int_lit(2)),
            val(
                "closed",
                binary(BinOp::RangeTo, int_lit(1), var("endpoint")),
            ),
            val(
                "open",
                binary(BinOp::RangeUntil, int_lit(1), var("endpoint")),
            ),
            val("until", infix_integer(int_lit(1), "until", var("endpoint"))),
            val(
                "downTo",
                infix_integer(int_lit(1), "downTo", var("endpoint")),
            ),
        ],
    )]);
    let module = lower(&[core_with_int8_ranges(), user])
        .map(|output| output.export)
        .expect("typed core range roles may constrain a literal receiver");
    let int8 = integer_type(&module, hir::IntegerKind::SIGNED_8);
    for name in ["closed", "open", "until", "downTo"] {
        assert_eq!(local_init(main_body(&module), name).ty, int8, "{name}");
    }
}
