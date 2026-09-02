use super::*;

// --- negative: type operators ---

#[test]
fn ref_eq_on_value_types_is_an_error() {
    let file = file(vec![
        struct_s(),
        fun_expr(
            "f",
            vec![],
            vec![("a", ty_named("S")), ("b", ty_named("S"))],
            Some(ty_named("Boolean")),
            binary(BinOp::RefEq, var("a"), var("b")),
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("`===` on value types must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "reference equality `===` is not supported on value types"
    );
}

#[test]
fn useless_is_check_is_an_error() {
    let file = file(vec![
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Boolean")),
            is_ty(var("x"), ty_named("String"), false),
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a useless check must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "useless type check: `Int` can never be `String`"
    );
}

#[test]
fn impossible_cast_is_an_error() {
    let file = file(vec![
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("String")),
            cast_ty(var("x"), ty_named("String"), false),
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("an impossible cast must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "cast from `Int` to `String` can never succeed"
    );
}

#[test]
fn mutable_variables_are_not_smart_cast() {
    let file = file(vec![
        struct_s(),
        fun_sig(
            "f",
            vec![],
            vec![],
            Some(ty_named("Int")),
            vec![
                // `var x: Any` — the mutable variable is not narrowed
                // inside the branch, so `x.v` still resolves against
                // `Any`.
                Statement {
                    kind: StatementKind::ValDecl(ValDecl {
                        mutable: true,
                        target: pat_bind("x"),
                        ty: Some(ty_named("Any")),
                        init: struct_init("S", vec![int_lit(1)]),
                        span: sp(),
                    }),
                    span: sp(),
                },
                if_stmt(
                    is_ty(var("x"), ty_named("S"), false),
                    vec![ret(Some(field(var("x"), "v")))],
                    None,
                ),
                ret(Some(int_lit(0))),
            ],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("a mutable variable must not narrow");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "type `Any` has no fields");
}
