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
        "no applicable candidate for `Array` in core prelude layer:\n  - no applicable candidate for constructor `Array` in nominal constructor candidate layer:\n  - class Array<T>(size: Long, init: (Long) -> T) — expects 2 argument(s), but 0 were supplied\n  - no applicable candidate for constructor `Array` in nominal constructor candidate layer:\n  - class Array<T>(source: MutableArray<T>) — expects 1 argument(s), but 0 were supplied"
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
        "no applicable candidate for `Array` in core prelude layer:\n  - no applicable candidate for constructor `Array` in nominal constructor candidate layer:\n  - class Array<T>(size: Long, init: (Long) -> T) — argument for `size` has type MutableArray<Int>, which is not a subtype of Long\n  - no applicable candidate for constructor `Array` in nominal constructor candidate layer:\n  - class Array<T>(source: MutableArray<T>) — expects 1 argument(s), but 2 were supplied"
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
        "no applicable candidate for `toMutableArray` in member candidate layer:\n  - fun Array<T>.toMutableArray(): MutableArray<T> — expects 0 argument(s), but 1 were supplied"
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
        "no applicable candidate for `Array` in core prelude layer:\n  - no applicable candidate for constructor `Array` in nominal constructor candidate layer:\n  - class Array<T>(size: Long, init: (Long) -> T) — expects 2 argument(s), but 1 were supplied\n  - no applicable candidate for constructor `Array` in nominal constructor candidate layer:\n  - class Array<T>(source: MutableArray<T>) — argument for `source` has type Array<Int>, which is not a subtype of MutableArray<T>"
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
        "no applicable candidate for `MutableArray` in core prelude layer:\n  - no applicable candidate for constructor `MutableArray` in nominal constructor candidate layer:\n  - class MutableArray<T>(size: Long, init: (Long) -> T) — expects 2 argument(s), but 1 were supplied\n  - no applicable candidate for constructor `MutableArray` in nominal constructor candidate layer:\n  - class MutableArray<T>(source: Array<T>) — argument for `source` has type MutableArray<Int>, which is not a subtype of Array<T>"
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
        "no applicable candidate for `Array` in core prelude layer:\n  - no applicable candidate for constructor `Array` in nominal constructor candidate layer:\n  - class Array<T>(size: Long, init: (Long) -> T) — expects 2 argument(s), but 1 were supplied\n  - no applicable candidate for constructor `Array` in nominal constructor candidate layer:\n  - class Array<T>(source: MutableArray<T>) — argument for `source` has type Int, which is not a subtype of MutableArray<T>"
    );

    let file2 = super::file(vec![fun(
        "main",
        vec![val("m", call("MutableArray", vec![int_lit(1)]))],
    )]);
    let errors = lower_user(file2).expect_err("`MutableArray(1)` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no applicable candidate for `MutableArray` in core prelude layer:\n  - no applicable candidate for constructor `MutableArray` in nominal constructor candidate layer:\n  - class MutableArray<T>(size: Long, init: (Long) -> T) — expects 2 argument(s), but 1 were supplied\n  - no applicable candidate for constructor `MutableArray` in nominal constructor candidate layer:\n  - class MutableArray<T>(source: Array<T>) — argument for `source` has type Int, which is not a subtype of Array<T>"
    );
}

#[test]
fn conversion_explicit_type_arguments_participate_in_constraints() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty(
                "m",
                Some(ty_mutable_int_array()),
                array_lit(vec![int_lit(1)]),
            ),
            val(
                "a",
                typed_call("Array", vec![ty_named("String")], vec![var("m")]),
            ),
        ],
    )]);
    let errors = lower_user(file).expect_err("the explicit element type must constrain the source");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "no applicable candidate for `Array` in core prelude layer:\n  - no applicable candidate for constructor `Array` in nominal constructor candidate layer:\n  - class Array<T>(size: Long, init: (Long) -> T) — expects 2 argument(s), but 1 were supplied\n  - no applicable candidate for constructor `Array` in nominal constructor candidate layer:\n  - class Array<T>(source: MutableArray<T>) — conflicting types for `T`: String and Int"
    );
}

#[test]
fn array_conversion_intrinsics_are_required_and_shape_checked() {
    let user = || file(vec![fun("main", vec![])]);

    let mut missing = core_file();
    let array = missing
        .declarations
        .iter_mut()
        .find_map(|declaration| match declaration {
            Decl::Class(class) if class.name.text == "Array" => Some(class),
            _ => None,
        })
        .expect("test core declares Array");
    array.members.retain(|member| {
        !matches!(member, ast::ClassMember::Function(method) if method.name.text == "toMutableArray")
    });
    let errors = lower(&[missing, user()]).expect_err("the array conversion intrinsic is required");
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("scoop.core must define exactly one `array_to_mutable` intrinsic")
    }));

    let mut malformed = core_file();
    let mutable_array = malformed
        .declarations
        .iter_mut()
        .find_map(|declaration| match declaration {
            Decl::Class(class) if class.name.text == "MutableArray" => Some(class),
            _ => None,
        })
        .expect("test core declares MutableArray");
    mutable_array
        .members
        .iter_mut()
        .filter_map(|member| match member {
            ast::ClassMember::Function(method) => Some(method),
            _ => None,
        })
        .find(|method| method.name.text == "toArray")
        .expect("test core declares MutableArray.toArray")
        .return_ty = Some(ty_named("String"));
    let errors = lower(&[malformed, user()]).expect_err("the array conversion signature is fixed");
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("malformed core array intrinsic `array_to_immutable`")
    }));
}
