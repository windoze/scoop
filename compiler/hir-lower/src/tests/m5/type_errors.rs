use super::*;

// --- negative: type annotations ---

#[test]
fn array_annotation_takes_exactly_one_type_argument() {
    let file = file(vec![fun(
        "main",
        vec![val_ty(
            "a",
            Some(ty_generic("Array", vec![ty_named("Int"), ty_named("Int")])),
            array_lit(vec![]),
        )],
    )]);
    let errors = lower_user(file).expect_err("wrong type arity must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "class `Array` takes 1 type argument(s), but 2 were supplied"
    );
}

#[test]
fn bare_array_annotation_requires_a_type_argument() {
    let file = file(vec![fun(
        "main",
        vec![val_ty("a", Some(ty_named("Array")), array_lit(vec![]))],
    )]);
    let errors = lower_user(file).expect_err("bare `Array` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "generic class `Array` requires 1 type argument(s)"
    );
}

#[test]
fn array_and_mutable_array_are_not_interchangeable() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty(
                "m",
                Some(ty_mutable_int_array()),
                array_lit(vec![int_lit(1)]),
            ),
            val_ty("a", Some(ty_int_array()), var("m")),
        ],
    )]);
    let errors = lower_user(file).expect_err("invariance must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "initializer of `a` must be of type Array<Int>, found MutableArray<Int>"
    );
}
