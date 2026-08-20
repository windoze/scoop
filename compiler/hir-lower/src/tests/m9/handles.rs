use super::*;

// --- The handle types carry their (phantom) type argument ---

#[test]
fn pin_result_matches_a_pinned_ptr_annotation() {
    let file = file(vec![fun(
        "main",
        vec![unsafe_block(vec![val_ty(
            "h",
            Some(ty_generic("PinnedPtr", vec![ty_named("String")])),
            call("pin", vec![str_lit("x")]),
        )])],
    )]);
    lower_user_with_gc(file).expect("matching handle types must lower");
}

#[test]
fn pinned_ptr_type_arguments_are_strict() {
    let file = file(vec![fun(
        "main",
        vec![val_ty(
            "h",
            Some(ty_generic("PinnedPtr", vec![ty_named("Int")])),
            call("pin", vec![str_lit("x")]),
        )],
    )]);
    let errors = lower_user_with_gc(file)
        .expect_err("PinnedPtr<Int> and PinnedPtr<String> are different types");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "type argument `Int` for `T` of struct `PinnedPtr` must satisfy `ref`"
    );
}

#[test]
fn bare_handle_type_requires_type_arguments() {
    let file = file(vec![
        fun_sig(
            "f",
            vec![],
            vec![("h", ty_named("PinnedPtr"))],
            None,
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user_with_gc(file).expect_err("a bare PinnedPtr must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "generic struct `PinnedPtr` requires 1 type argument(s)"
    );
}

// --- PinnedPtr as a generic struct: construction and field access ---

#[test]
fn pinned_ptr_construction_and_field_access() {
    let file = file(vec![fun(
        "main",
        vec![unsafe_block(vec![
            val("u", call("gcStats", vec![])),
            val_ty(
                "h",
                Some(ty_generic("PinnedPtr", vec![ty_named("String")])),
                struct_init("PinnedPtr", vec![var("u")]),
            ),
            val("r", field(var("h"), "raw")),
            // Field access also works on a `PinnedPtr<T>` application
            // (the `raw` field is an ordinary struct field).
            val_ty(
                "r2",
                Some(ty_named("ULong")),
                field(call("pin", vec![str_lit("x")]), "raw"),
            ),
        ])],
    )]);
    let module = lower_user_with_gc(file).expect("handle construction must lower");
    assert_eq!(local_ty(&module, "h"), "PinnedPtr<String>");
    assert_eq!(local_ty(&module, "r"), "ULong");
    assert_eq!(local_ty(&module, "r2"), "ULong");
}

#[test]
fn pinned_ptr_construction_checks_the_raw_field() {
    let file = file(vec![fun(
        "main",
        vec![val_ty(
            "h",
            Some(ty_generic("PinnedPtr", vec![ty_named("String")])),
            struct_init("PinnedPtr", vec![int_lit(1)]),
        )],
    )]);
    let errors = lower_user_with_gc(file).expect_err("an Int `raw` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "integer literal `1` is not representable as ULong"
    );
}

#[test]
fn gcstats_result_is_uint_not_int() {
    let file = file(vec![fun(
        "main",
        vec![val_ty("n", Some(ty_named("Int")), call("gcStats", vec![]))],
    )]);
    let errors = lower_user_with_gc(file).expect_err("gcStats does not return Int");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "initializer of `n` must be of type Int, found ULong"
    );
}
