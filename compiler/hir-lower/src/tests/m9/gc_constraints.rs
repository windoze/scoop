use super::*;

// --- GC intrinsics: the reference-type constraint ---

#[test]
fn pin_rejects_an_int_argument() {
    let file = file(vec![fun(
        "main",
        vec![val("h", call("pin", vec![int_lit(42)]))],
    )]);
    let errors = lower_user_with_gc(file).expect_err("pinning a value type must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "type argument `Int` for `T` of function `pin` must satisfy `ref`"
    );
    // The user file is index 2 (core files are 0 and 1).
    assert_eq!(errors[0].file, 2);
}

#[test]
fn pin_rejects_a_struct_argument() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![val(
                "h",
                call("pin", vec![struct_init("Point", vec![int_lit(1)])]),
            )],
        ),
    ]);
    let errors = lower_user_with_gc(file).expect_err("pinning a struct must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "type argument `Point` for `T` of function `pin` must satisfy `ref`"
    );
}

#[test]
fn get_gc_handle_rejects_a_value_argument() {
    let file = file(vec![fun(
        "main",
        vec![val("g", call("getGcHandle", vec![int_lit(42)]))],
    )]);
    let errors = lower_user_with_gc(file).expect_err("a handle of a value type must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "type argument `Int` for `T` of function `getGcHandle` must satisfy `ref`"
    );
}

#[test]
fn unpin_rejects_a_value_type_parameter() {
    let file = file(vec![
        fun_sig(
            "bad",
            vec![],
            vec![("h", ty_generic("PinnedPtr", vec![ty_named("Int")]))],
            None,
            vec![stmt(call("unpin", vec![var("h")]))],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user_with_gc(file).expect_err("unpin of PinnedPtr<Int> must fail");
    assert!(errors.iter().any(|error| error.message
        == "type argument `Int` for `T` of struct `PinnedPtr` must satisfy `ref`"));
}

#[test]
fn release_gc_handle_rejects_a_value_type_parameter() {
    let file = file(vec![
        fun_sig(
            "bad",
            vec![],
            vec![("h", ty_generic("GcHandle", vec![ty_named("Int")]))],
            None,
            vec![stmt(call("releaseGcHandle", vec![var("h")]))],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user_with_gc(file).expect_err("releaseGcHandle of GcHandle<Int> must fail");
    assert!(errors.iter().any(|error| error.message
        == "type argument `Int` for `T` of struct `GcHandle` must satisfy `ref`"));
}

#[test]
fn pin_rejects_an_unconstrained_type_parameter() {
    // An unconstrained parameter may still be instantiated with a value
    // type, so it cannot satisfy the core API's M12 `T : ref` bound.
    let file = file(vec![
        fun_sig(
            "f",
            vec!["U"],
            vec![("u", ty_named("U"))],
            None,
            vec![stmt(call("pin", vec![var("u")]))],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user_with_gc(file).expect_err("pinning an unconstrained T must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "type argument `U` for `T` of function `pin` must satisfy `ref`"
    );
}

#[test]
fn pin_accepts_class_array_any_and_string_references() {
    let file = file(vec![fun(
        "main",
        vec![unsafe_block(vec![
            val("a", call("pin", vec![call("Throwable", vec![])])),
            val("b", call("pin", vec![array_lit(vec![int_lit(1)])])),
            val("c", call("pin", vec![str_lit("x")])),
            val("d", call("unpin", vec![call("pin", vec![str_lit("y")])])),
        ])],
    )]);
    let module = lower_user_with_gc(file).expect("reference arguments must lower");
    // The nested `unpin(pin("y"))` binds T = String through the
    // PinnedPtr<String> application produced by the inner call.
    assert_eq!(local_ty(&module, "d"), "String");
}
