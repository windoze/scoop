use super::*;

// --- positive: golden dumps ---

/// Literals in every inference mode, subscript read/write, `.size` on
/// both kinds and both conversion directions.
#[test]
fn array_basics_golden() {
    let file = file(vec![fun(
        "main",
        vec![
            // No expectation: elements share a type, result is `Array`.
            val("a", array_lit(vec![int_lit(1), int_lit(2), int_lit(3)])),
            // A `MutableArray` annotation picks the kind.
            val_ty(
                "m",
                Some(ty_mutable_int_array()),
                array_lit(vec![int_lit(4), int_lit(5)]),
            ),
            // An empty literal is legal with an expected type.
            val_ty("e", Some(ty_int_array()), array_lit(vec![])),
            val("first", subscript(var("a"), int_lit(0))),
            val("n", field(var("a"), "size")),
            val("s", field(var("m"), "size")),
            assign_index(var("m"), int_lit(0), int_lit(40)),
            val("b", call("Array", vec![var("m")])),
            val("m2", call("MutableArray", vec![var("a")])),
            // Nested literals infer `Array<Array<Int>>`.
            val(
                "nested",
                array_lit(vec![
                    array_lit(vec![int_lit(1), int_lit(2)]),
                    array_lit(vec![int_lit(3)]),
                ]),
            ),
        ],
    )]);
    let module = lower_user(file).expect("array program must lower");
    let expected = include_str!("snapshots/array_basics_golden.hir.txt");
    assert_eq!(hir::dump(&module), expected);
}

/// A generic function over array elements: `T` is inferred from the
/// literal's element type through the `Array<T>` parameter.
#[test]
fn generic_function_over_array_elements() {
    let file = file(vec![
        fun_expr(
            "first",
            vec!["T"],
            vec![("a", ty_generic("Array", vec![ty_named("T")]))],
            Some(ty_named("T")),
            subscript(var("a"), int_lit(0)),
        ),
        fun(
            "main",
            vec![val(
                "x",
                call("first", vec![array_lit(vec![int_lit(1), int_lit(2)])]),
            )],
        ),
    ]);
    let module = lower_user(file).expect("generic array program must lower");
    let expected = include_str!("snapshots/generic_function_over_array_elements.hir.txt");
    assert_eq!(hir::dump(&module), expected);
}
