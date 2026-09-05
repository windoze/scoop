use super::*;

fn if_expr(condition: Expr, then_value: Expr, else_value: Expr) -> Expr {
    Expr::If(Box::new(ast::If {
        cond: condition,
        then_block: block(vec![stmt(then_value)]),
        else_block: Some(block(vec![stmt(else_value)])),
        span: sp(),
    }))
}

fn when_expr(subject: Expr, arms: Vec<ast::WhenArm>) -> Expr {
    Expr::When(Box::new(ast::When {
        subject,
        arms,
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

fn assert_option_int(module: &hir::Module, local: &str) {
    assert_eq!(
        hir::type_name(
            module,
            local_init(
                match &module.functions[module.entry].kind {
                    hir::FunctionKind::User(body) => body,
                    _ => panic!("main must have a user body"),
                },
                local,
            )
            .ty
        ),
        "Option<Int>"
    );
}

#[test]
fn nominal_fixed_points_use_defaultable_literals_after_contextual_inputs_stall() {
    let module = lower_user(file(vec![
        generic_struct_decl(
            "Pair",
            vec!["T"],
            vec![
                ("maybe", ty_nullable(ty_named("T"))),
                ("value", ty_named("T")),
            ],
        ),
        enum_decl(
            "Duo",
            vec!["T"],
            vec![variant_positional(
                "Both",
                vec![ty_nullable(ty_named("T")), ty_named("T")],
            )],
        ),
        fun(
            "main",
            vec![
                val("pair", struct_init("Pair", vec![none(), int_lit(8)])),
                val("duo", struct_init("Duo.Both", vec![none(), int_lit(9)])),
            ],
        ),
    ]))
    .expect("a defaultable literal must seed a stalled nominal fixed point");
    let body = match &module.functions[module.entry].kind {
        hir::FunctionKind::User(body) => body,
        _ => panic!("main must have a user body"),
    };
    assert_eq!(
        hir::type_name(&module, local_init(body, "pair").ty),
        "Pair<Int>"
    );
    assert_eq!(
        hir::type_name(&module, local_init(body, "duo").ty),
        "Duo<Int>"
    );
}

#[test]
fn control_value_fixed_points_use_nested_integer_defaults_as_seeds() {
    let module = lower_user(file(vec![
        enum_decl(
            "Choice",
            Vec::new(),
            vec![variant_unit("First"), variant_unit("Second")],
        ),
        fun(
            "main",
            vec![
                val("fromIf", if_expr(bool_lit(false), none(), some(int_lit(4)))),
                val(
                    "fromWhen",
                    when_expr(
                        struct_init("Choice.First", Vec::new()),
                        vec![
                            arm(pat_bind("First"), None, vec![stmt(none())]),
                            arm(pat_bind("Second"), None, vec![stmt(some(int_lit(5)))]),
                        ],
                    ),
                ),
                val("fromTry", try_expr(some(int_lit(9)), none())),
            ],
        ),
    ]))
    .expect("defaultable branch literals must seed stalled control-value inference");
    for local in ["fromIf", "fromWhen", "fromTry"] {
        assert_option_int(&module, local);
    }
}

#[test]
fn nested_literal_constructors_remain_contextual_to_each_overload_candidate() {
    let errors = lower_user(file(vec![
        fun_expr(
            "select",
            Vec::new(),
            vec![("value", ty_nullable(ty_named("Int8")))],
            Some(ty_named("Unit")),
            unit_lit(),
        ),
        fun_expr(
            "select",
            Vec::new(),
            vec![("value", ty_nullable(ty_named("Int16")))],
            Some(ty_named("Unit")),
            unit_lit(),
        ),
        fun("main", vec![stmt(call("select", vec![some(int_lit(1))]))]),
    ]))
    .expect_err("both candidate-local nested literal probes must remain applicable");
    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.message.contains("call to `select` is ambiguous")),
        "{errors:#?}"
    );
}
