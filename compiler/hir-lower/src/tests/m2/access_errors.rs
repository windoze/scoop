use super::super::*;

// --- negative: field access ---

#[test]
fn unknown_struct_field_is_an_error() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("p", call("Point", vec![int_lit(1)])),
                val("z", field(var("p"), "z")),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("unknown field must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "struct `Point` has no field `z`");
}

#[test]
fn struct_index_access_is_an_error() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("p", call("Point", vec![int_lit(1)])),
                val("z", index(var("p"), 1)),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("index on struct must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "struct `Point` has no field `_1`");
}

#[test]
fn tuple_index_out_of_bounds_is_an_error() {
    for n in [0, 3] {
        let file = file(vec![fun(
            "main",
            vec![
                val("q", tuple_lit(vec![int_lit(1), str_lit("s")])),
                val("z", index(var("q"), n)),
            ],
        )]);
        let errors = lower_user(file).expect_err("out-of-bounds index must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            format!("tuple type `(Int, String)` has no element `_{n}`")
        );
    }
}

#[test]
fn tuple_named_field_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val("q", tuple_lit(vec![int_lit(1), str_lit("s")])),
            val("z", field(var("q"), "x")),
        ],
    )]);
    let errors = lower_user(file).expect_err("named access on tuple must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "tuple type `(Int, String)` has no field `x`"
    );
}

#[test]
fn field_access_on_scalar_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val("n", int_lit(1)), val("z", field(var("n"), "x"))],
    )]);
    let errors = lower_user(file).expect_err("field access on Int must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "type `Int` has no fields");
}

// --- negative: statements ---

#[test]
fn struct_init_statement_is_not_a_call() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun("main", vec![stmt(call("Point", vec![int_lit(1)]))]),
    ]);
    let errors = lower_user(file).expect_err("construction statement must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "statement must be a function call");
}

#[test]
fn empty_tuple_literal_is_an_error() {
    let file = file(vec![fun("main", vec![val("t", tuple_lit(vec![]))])]);
    let errors = lower_user(file).expect_err("empty tuple literal must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "tuple literal must contain at least one element"
    );
}
