use super::*;

// --- negative: conversions ---

#[test]
fn conversion_takes_exactly_one_argument() {
    let m = || {
        val_ty(
            "m",
            Some(ty_mutable_int_array()),
            array_lit(vec![int_lit(1)]),
        )
    };
    // Zero arguments.
    let file = file(vec![fun(
        "main",
        vec![m(), val("b", call("Array", vec![]))],
    )]);
    let errors = lower_user(file).expect_err("`Array()` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`Array` takes exactly 1 argument, but 0 were supplied"
    );
    // Two arguments.
    let file2 = super::file(vec![fun(
        "main",
        vec![m(), val("b", call("Array", vec![var("m"), var("m")]))],
    )]);
    let errors = lower_user(file2).expect_err("`Array(m, m)` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`Array` takes exactly 1 argument, but 2 were supplied"
    );
}

#[test]
fn conversion_methods_take_no_arguments_and_only_exist_on_the_source_kind() {
    let file = file(vec![fun(
        "main",
        vec![
            val("a", array_lit(vec![int_lit(1)])),
            val(
                "m",
                method_call(var("a"), "toMutableArray", vec![int_lit(2)]),
            ),
        ],
    )]);
    let errors = lower_user(file).expect_err("conversion method arguments must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "method `toMutableArray` takes exactly 0 arguments, but 1 were supplied"
    );

    let file2 = super::file(vec![fun(
        "main",
        vec![
            val("a", array_lit(vec![int_lit(1)])),
            val("same", method_call(var("a"), "toArray", vec![])),
        ],
    )]);
    let errors = lower_user(file2).expect_err("same-kind method conversion must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "type `Array<Int>` has no method `toArray`"
    );
}

#[test]
fn conversion_needs_the_other_array_kind() {
    // `Array(x)` where `x` is already an `Array`.
    let file = file(vec![fun(
        "main",
        vec![
            val("a", array_lit(vec![int_lit(1)])),
            val("b", call("Array", vec![var("a")])),
        ],
    )]);
    let errors = lower_user(file).expect_err("same-kind conversion must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "use the value directly; conversion is only between Array and MutableArray"
    );

    // `MutableArray(x)` where `x` is already a `MutableArray`.
    let file2 = super::file(vec![fun(
        "main",
        vec![
            val_ty(
                "m",
                Some(ty_mutable_int_array()),
                array_lit(vec![int_lit(1)]),
            ),
            val("m2", call("MutableArray", vec![var("m")])),
        ],
    )]);
    let errors = lower_user(file2).expect_err("same-kind conversion must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "use the value directly; conversion is only between Array and MutableArray"
    );
}

#[test]
fn conversion_argument_must_be_an_array() {
    let file = file(vec![fun(
        "main",
        vec![val("b", call("Array", vec![int_lit(1)]))],
    )]);
    let errors = lower_user(file).expect_err("`Array(1)` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "argument of `Array` conversion must be a MutableArray, found Int"
    );

    let file2 = super::file(vec![fun(
        "main",
        vec![val("m", call("MutableArray", vec![int_lit(1)]))],
    )]);
    let errors = lower_user(file2).expect_err("`MutableArray(1)` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "argument of `MutableArray` conversion must be an Array, found Int"
    );
}
