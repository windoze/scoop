use super::super::*;

// --- negative: struct declarations ---

#[test]
fn duplicate_struct_is_an_error() {
    let file = file(vec![
        struct_decl("Point", vec![]),
        struct_decl("Point", vec![]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("duplicate struct must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate struct `Point`");
}

#[test]
fn duplicate_field_is_an_error() {
    let file = file(vec![
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("x", ty_named("Int"))],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("duplicate field must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate field `x` in struct `Point`");
}

#[test]
fn unknown_field_type_is_an_error() {
    let file = file(vec![
        struct_decl("Point", vec![("z", ty_named("Foo"))]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("unknown field type must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown type `Foo`");
}

// --- negative: struct construction ---

#[test]
fn struct_init_arity_is_an_error() {
    for (args, expected, supplied) in [
        (vec![int_lit(1)], 2, 1),
        (vec![int_lit(1), int_lit(2), int_lit(3)], 2, 3),
    ] {
        let file = file(vec![
            struct_decl(
                "Point",
                vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
            ),
            fun("main", vec![val("p", call("Point", args))]),
        ]);
        let errors = lower_user(file).expect_err("wrong arity must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            format!(
                "struct `Point` takes exactly {expected} arguments, but {supplied} were supplied"
            )
        );
    }
}

#[test]
fn struct_init_arity_singular_noun() {
    let file = file(vec![
        struct_decl("Box", vec![("v", ty_named("Int"))]),
        fun("main", vec![val("b", call("Box", vec![]))]),
    ]);
    let errors = lower_user(file).expect_err("wrong arity must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "struct `Box` takes exactly 1 argument, but 0 were supplied"
    );
}

#[test]
fn struct_init_field_type_is_an_error() {
    let file = file(vec![
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
        ),
        fun(
            "main",
            vec![val("p", call("Point", vec![int_lit(1), str_lit("s")]))],
        ),
    ]);
    let errors = lower_user(file).expect_err("field type mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "argument for field `y` of `Point` must be of type Int, found String"
    );
}

#[test]
fn unknown_struct_init_node_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val("p", struct_init("Foo", vec![]))],
    )]);
    let errors = lower_user(file).expect_err("unknown struct must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown struct `Foo`");
}
