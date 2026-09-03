use super::*;

// --- negative: subscript read ---

#[test]
fn subscript_requires_an_array_receiver() {
    let file = file(vec![fun(
        "main",
        vec![
            val("x", int_lit(1)),
            val("y", subscript(var("x"), int_lit(0))),
        ],
    )]);
    let errors = lower_user(file).expect_err("subscript on Int must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "subscript is only supported on arrays, found Int"
    );
}

#[test]
fn subscript_index_must_be_int() {
    let file = file(vec![fun(
        "main",
        vec![
            val("a", array_lit(vec![int_lit(1)])),
            val("y", subscript(var("a"), str_lit("x"))),
        ],
    )]);
    let errors = lower_user(file).expect_err("non-Int index must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "array index must be Int, found String");
}

#[test]
fn size_is_only_a_pseudo_property_of_arrays() {
    // `.size` on a struct is the ordinary unknown-field diagnostic.
    let file = file(vec![
        struct_decl("P", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("p", struct_init("P", vec![int_lit(1)])),
                val("s", field(var("p"), "size")),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("struct `.size` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "struct `P` has no field `size`");

    // `.size` on an Int: no fields at all.
    let file2 = super::file(vec![fun(
        "main",
        vec![val("x", int_lit(1)), val("s", field(var("x"), "size"))],
    )]);
    let errors = lower_user(file2).expect_err("Int `.size` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "type `Int` has no fields");
}

// --- negative: subscript write ---

#[test]
fn subscript_write_requires_a_mutable_array() {
    // An immutable `Array` — even behind a `var` binding, since the
    // kind is a property of the type, not of the binding.
    for mutable_binding in [false, true] {
        let declaration = if mutable_binding {
            var_("a", array_lit(vec![int_lit(1), int_lit(2)]))
        } else {
            val("a", array_lit(vec![int_lit(1), int_lit(2)]))
        };
        let file = file(vec![fun(
            "main",
            vec![declaration, assign_index(var("a"), int_lit(0), int_lit(3))],
        )]);
        let errors = lower_user(file).expect_err("write to `Array` must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "cannot assign to an element of immutable Array<Int>"
        );
    }
}

#[test]
fn subscript_write_on_a_non_array_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            var_("x", int_lit(1)),
            assign_index(var("x"), int_lit(0), int_lit(2)),
        ],
    )]);
    let errors = lower_user(file).expect_err("write through Int must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "subscript is only supported on arrays, found Int"
    );
}

#[test]
fn subscript_write_index_must_be_int() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty(
                "m",
                Some(ty_mutable_int_array()),
                array_lit(vec![int_lit(1)]),
            ),
            assign_index(var("m"), str_lit("x"), int_lit(2)),
        ],
    )]);
    let errors = lower_user(file).expect_err("non-Int write index must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "array index must be Int, found String");
}

#[test]
fn subscript_write_value_must_match_the_element_type() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty(
                "m",
                Some(ty_mutable_int_array()),
                array_lit(vec![int_lit(1)]),
            ),
            assign_index(var("m"), int_lit(0), str_lit("x")),
        ],
    )]);
    let errors = lower_user(file).expect_err("value mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "cannot assign value of type String to an array element of type Int"
    );
}
