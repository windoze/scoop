use super::*;

fn infix_integer(lhs: Expr, name: &str, rhs: Expr) -> Expr {
    Expr::InfixCall {
        lhs: Box::new(lhs),
        target: ast::InfixTarget::Named(ident(name)),
        rhs: Box::new(rhs),
        span: sp(),
    }
}

fn suffixed_integer(magnitude: u64, suffix: ast::IntegerSuffix) -> Expr {
    Expr::IntLiteral(ast::IntegerLiteralSyntax {
        magnitude,
        radix: ast::IntegerRadix::Decimal,
        suffix,
        span: sp(),
    })
}

fn main_body(module: &hir::ExportHirOutput) -> &hir::Body {
    let hir::FunctionKind::User(body) = &module.functions[module.entry()].kind else {
        panic!("main must have a user body")
    };
    body
}

#[test]
fn integer_literal_receiver_uses_the_exact_peer_kind_in_both_directions() {
    let module = lower_user(file(vec![fun(
        "main",
        vec![
            val_ty("small", Some(ty_named("Int8")), int_lit(3)),
            val("leftAdd", binary(BinOp::Add, int_lit(1), var("small"))),
            val("rightAdd", binary(BinOp::Add, var("small"), int_lit(1))),
            val("leftBit", infix_integer(int_lit(1), "and", var("small"))),
            val("rightBit", infix_integer(var("small"), "and", int_lit(1))),
            val("leftEq", binary(BinOp::Eq, int_lit(1), var("small"))),
            val("rightEq", binary(BinOp::Eq, var("small"), int_lit(1))),
        ],
    )]))
    .expect("integer literal peers must constrain the closed integer family");
    let body = main_body(&module);
    let int8 = integer_type(&module, hir::IntegerKind::SIGNED_8);
    assert_eq!(local_init(body, "leftAdd").ty, int8);
    assert_eq!(local_init(body, "rightAdd").ty, int8);
    assert_eq!(local_init(body, "leftBit").ty, int8);
    assert_eq!(local_init(body, "rightBit").ty, int8);
    assert_eq!(local_init(body, "leftEq").ty, module.boolean);
    assert_eq!(local_init(body, "rightEq").ty, module.boolean);
}

#[test]
fn explicit_integer_member_calls_delay_a_literal_receiver_within_the_closed_family() {
    let module = lower_user(file(vec![fun(
        "main",
        vec![
            val_ty("small", Some(ty_named("Int8")), int_lit(3)),
            val_ty(
                "inverse",
                Some(ty_named("Int8")),
                method_call(int_lit(1), "inv", Vec::new()),
            ),
            val_ty(
                "sum",
                Some(ty_named("Int8")),
                method_call(int_lit(1), "plus", vec![int_lit(2)]),
            ),
            val_ty(
                "incremented",
                Some(ty_named("Int8")),
                method_call(int_lit(1), "inc", Vec::new()),
            ),
            val(
                "peerConstrained",
                method_call(int_lit(1), "plus", vec![var("small")]),
            ),
        ],
    )]))
    .expect("explicit integer members must not default a literal receiver before resolution");
    let body = main_body(&module);
    let int8 = integer_type(&module, hir::IntegerKind::SIGNED_8);
    for name in ["inverse", "sum", "incremented", "peerConstrained"] {
        assert_eq!(local_init(body, name).ty, int8);
    }
}

#[test]
fn overload_resolution_prefers_the_literal_default_kind_only_when_present() {
    let module = lower_user(file(vec![
        fun_expr(
            "select",
            Vec::new(),
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            int_lit(32),
        ),
        fun_expr(
            "select",
            Vec::new(),
            vec![("value", ty_named("Long"))],
            Some(ty_named("Int")),
            int_lit(64),
        ),
        fun_expr(
            "selectUnsigned",
            Vec::new(),
            vec![("value", ty_named("UInt"))],
            Some(ty_named("Int")),
            int_lit(32),
        ),
        fun_expr(
            "selectUnsigned",
            Vec::new(),
            vec![("value", ty_named("ULong"))],
            Some(ty_named("Int")),
            int_lit(64),
        ),
        fun(
            "main",
            vec![
                val("signed", call("select", vec![int_lit(1)])),
                val(
                    "unsigned",
                    call(
                        "selectUnsigned",
                        vec![suffixed_integer(1, ast::IntegerSuffix::Unsigned)],
                    ),
                ),
            ],
        ),
    ]))
    .expect("literal default kinds must break only the default-width overload tie");
    let body = main_body(&module);
    for (name, expected_parameter) in [("signed", "Int"), ("unsigned", "UInt")] {
        let hir::ExprKind::Call {
            callee: hir::CallableTarget::Local(callee),
            ..
        } = local_init(body, name).kind
        else {
            panic!("selection must be a direct call")
        };
        let function = &module.functions[module.callable_function(callee)];
        assert_eq!(
            hir::type_name(&module, function.params[0].ty),
            expected_parameter
        );
    }

    let errors = lower_user(file(vec![
        fun_expr(
            "narrow",
            Vec::new(),
            vec![("value", ty_named("Int8"))],
            Some(ty_named("Unit")),
            unit_lit(),
        ),
        fun_expr(
            "narrow",
            Vec::new(),
            vec![("value", ty_named("Int16"))],
            Some(ty_named("Unit")),
            unit_lit(),
        ),
        fun("main", vec![stmt(call("narrow", vec![int_lit(1)]))]),
    ]))
    .expect_err("multiple non-default literal fits must remain ambiguous");
    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.message.contains("call to `narrow` is ambiguous"))
    );
}

#[test]
fn literal_default_preference_is_a_per_argument_pareto_tie_break() {
    let module = lower_user(file(vec![
        fun_expr(
            "pick",
            Vec::new(),
            vec![("first", ty_named("Int")), ("second", ty_named("Int8"))],
            Some(ty_named("Int")),
            int_lit(1),
        ),
        fun_expr(
            "pick",
            Vec::new(),
            vec![("first", ty_named("Int16")), ("second", ty_named("Int8"))],
            Some(ty_named("Int")),
            int_lit(2),
        ),
        fun(
            "main",
            vec![val("selected", call("pick", vec![int_lit(1), int_lit(1)]))],
        ),
    ]))
    .expect("one differing default-exact literal parameter must dominate");
    let hir::ExprKind::Call {
        callee: hir::CallableTarget::Local(callee),
        ..
    } = local_init(main_body(&module), "selected").kind
    else {
        panic!("selected must be a direct call")
    };
    let selected = &module.functions[module.callable_function(callee)];
    assert_eq!(hir::type_name(&module, selected.params[0].ty), "Int");
    assert_eq!(hir::type_name(&module, selected.params[1].ty), "Int8");

    let errors = lower_user(file(vec![
        fun_expr(
            "cross",
            Vec::new(),
            vec![("first", ty_named("Int")), ("second", ty_named("Int16"))],
            Some(ty_named("Unit")),
            unit_lit(),
        ),
        fun_expr(
            "cross",
            Vec::new(),
            vec![("first", ty_named("Int16")), ("second", ty_named("Int"))],
            Some(ty_named("Unit")),
            unit_lit(),
        ),
        fun(
            "main",
            vec![stmt(call("cross", vec![int_lit(1), int_lit(1)]))],
        ),
    ]))
    .expect_err("crossed literal defaults must remain ambiguous");
    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.message.contains("call to `cross` is ambiguous"))
    );
}

#[test]
fn signed_minimum_literal_is_candidate_local_but_one_past_minimum_is_rejected() {
    let minimum = unary(UnOp::Neg, int_lit(128));
    let module = lower_user(file(vec![fun(
        "main",
        vec![val_ty("minimum", Some(ty_named("Int8")), minimum)],
    )]))
    .expect("the signed minimum magnitude belongs to the unary-minus literal boundary");
    assert!(matches!(
        local_init(main_body(&module), "minimum").kind,
        hir::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed8(0x80))
    ));

    let errors = lower_user(file(vec![fun(
        "main",
        vec![val_ty(
            "invalid",
            Some(ty_named("Int8")),
            unary(UnOp::Neg, int_lit(129)),
        )],
    )]))
    .expect_err("one past the signed minimum must be rejected");
    assert!(
        errors.iter().any(|diagnostic| diagnostic.message
            == "integer literal `-129` is not representable as Int8")
    );
}

#[test]
fn unsigned_unary_minus_uses_the_typed_wrapping_operation_and_pattern_constant() {
    let negative_unsigned = || unary(UnOp::Neg, suffixed_integer(1, ast::IntegerSuffix::Unsigned));
    let module = lower_user(file(vec![fun(
        "main",
        vec![
            val("runtime", negative_unsigned()),
            val_ty(
                "subject",
                Some(ty_named("UInt8")),
                suffixed_integer(255, ast::IntegerSuffix::Unsigned),
            ),
            when_stmt(
                var("subject"),
                vec![arm(pat_lit(negative_unsigned()), None, Vec::new())],
                Some(Vec::new()),
            ),
        ],
    )]))
    .expect("unsigned unary minus must wrap before literal-pattern matching");
    let body = main_body(&module);
    assert!(matches!(
        local_init(body, "runtime").kind,
        hir::ExprKind::IntegerOperation {
            operation: hir::IntegerOperation::NoGc {
                kind: hir::IntegerKind::UNSIGNED_32,
                operation: hir::NoGcIntegerOperation::UnaryMinus,
                ..
            },
            ..
        }
    ));
    let when = body
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::StatementKind::When(when) => Some(when),
            _ => None,
        })
        .expect("main contains a when statement");
    assert!(matches!(
        &when.arms[0].pattern,
        hir::Pattern::Literal {
            value: hir::Expr {
                kind: hir::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Unsigned8(255)),
                ..
            },
            equality: hir::LiteralPatternEquality::Integer {
                kind: hir::IntegerKind::UNSIGNED_8,
                ..
            },
            ..
        }
    ));
}

#[test]
fn literal_pattern_equality_plan_separates_every_integer_kind_from_ordinary_literals() {
    let mut statements = Vec::new();
    for (index, kind) in hir::IntegerKind::ALL.into_iter().enumerate() {
        let name = format!("integer{index}");
        let (initial, pattern) = match kind {
            hir::IntegerKind::SIGNED_8 => (int_lit(1), int_lit(1)),
            hir::IntegerKind::UNSIGNED_8 => (
                suffixed_integer(255, ast::IntegerSuffix::Unsigned),
                unary(
                    ast::UnOp::Neg,
                    suffixed_integer(1, ast::IntegerSuffix::Unsigned),
                ),
            ),
            kind if kind.signedness() == hir::IntegerSignedness::Signed => (int_lit(1), int_lit(1)),
            _ => {
                let literal = suffixed_integer(1, ast::IntegerSuffix::Unsigned);
                (literal.clone(), literal)
            }
        };
        statements.push(val_ty(
            &name,
            Some(ty_named(kind.canonical_name())),
            initial,
        ));
        statements.push(when_stmt(
            var(&name),
            vec![arm(pat_lit(pattern), None, Vec::new())],
            Some(Vec::new()),
        ));
    }
    statements.extend([
        val(
            "ordinarySubject",
            tuple_lit(vec![bool_lit(false), str_lit("x")]),
        ),
        when_stmt(
            var("ordinarySubject"),
            vec![arm(
                pat_tuple(vec![pat_lit(bool_lit(false)), pat_wild()], None),
                None,
                Vec::new(),
            )],
            Some(Vec::new()),
        ),
        when_stmt(
            var("ordinarySubject"),
            vec![arm(
                pat_tuple(vec![pat_wild(), pat_lit(str_lit("x"))], None),
                None,
                Vec::new(),
            )],
            Some(Vec::new()),
        ),
    ]);

    let module = lower_user(file(vec![fun("main", statements)]))
        .expect("literal patterns select complete typed equality plans");
    let patterns = main_body(&module)
        .statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            hir::StatementKind::When(when) => Some(&when.arms[0].pattern),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(patterns.len(), 10);

    for (pattern, expected_kind) in patterns.iter().take(8).zip(hir::IntegerKind::ALL) {
        let hir::Pattern::Literal {
            equality: hir::LiteralPatternEquality::Integer { kind },
            ..
        } = pattern
        else {
            panic!("integer literal patterns use their typed intrinsic plan");
        };
        assert_eq!(*kind, expected_kind);
    }

    let ordinary_patterns = patterns[8..].iter().enumerate().map(|(index, pattern)| {
        let hir::Pattern::Tuple(elements) = pattern else {
            panic!("ordinary primitive literals are nested in tuple patterns");
        };
        &elements[index]
    });
    for (pattern, expected_name) in ordinary_patterns.zip(["Boolean.equals", "String.equals"]) {
        let hir::Pattern::Literal {
            equality:
                hir::LiteralPatternEquality::Ordinary {
                    equals: hir::CallableTarget::Local(equals),
                },
            ..
        } = pattern
        else {
            panic!("Boolean and String literal patterns retain an ordinary callable");
        };
        assert_eq!(
            module.functions[module.callable_function(*equals)].name,
            expected_name
        );
    }
}

#[test]
fn unary_plus_is_not_an_integer_literal_pattern() {
    let source = file(vec![fun(
        "main",
        vec![
            val_ty("value", Some(ty_named("Int")), int_lit(1)),
            when_stmt(
                var("value"),
                vec![arm(
                    pat_lit(unary(ast::UnOp::Plus, int_lit(1))),
                    None,
                    Vec::new(),
                )],
                Some(Vec::new()),
            ),
        ],
    )]);
    let diagnostics = lower_user(source).expect_err("unary plus is not literal-pattern syntax");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].message, "expected a literal pattern");
}
