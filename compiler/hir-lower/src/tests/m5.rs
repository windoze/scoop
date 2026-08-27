//! M5 tests: `Array<T>` / `MutableArray<T>` annotations, array literal
//! inference (with and without an expected type, empty literals),
//! subscript reads and writes, `.size`, and the `Array(m)` /
//! `MutableArray(a)` conversions. Golden dumps lock the output
//! structure; one negative test per diagnostic.

use super::*;

/// The `MutableArray<Int>` annotation, used by several tests.
fn ty_mutable_int_array() -> TypeRef {
    ty_generic("MutableArray", vec![ty_named("Int")])
}

fn ty_int_array() -> TypeRef {
    ty_generic("Array", vec![ty_named("Int")])
}

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
    let expected = "\
Module
  enum Option<T>
    Some(_1: T0)
    None()
  fun print(): Unit <intrinsic rt_print>
  fun println(): Unit <intrinsic rt_println>
  fun main(): Unit
    val local0
      ArrayLiteral : Array<Int>
        IntLiteral 1 : Int
        IntLiteral 2 : Int
        IntLiteral 3 : Int
    val local1
      ArrayLiteral : MutableArray<Int>
        IntLiteral 4 : Int
        IntLiteral 5 : Int
    val local2
      ArrayLiteral : Array<Int>
    val local3
      Index : Int
        Local a : Array<Int>
        IntLiteral 0 : Int
    val local4
      ArrayLen : Int
        Local a : Array<Int>
    val local5
      ArrayLen : Int
        Local m : MutableArray<Int>
    assign []
      Local m : MutableArray<Int>
      IntLiteral 0 : Int
      IntLiteral 40 : Int
    val local6
      ArrayClone : Array<Int>
        Local m : MutableArray<Int>
    val local7
      ArrayClone : MutableArray<Int>
        Local a : Array<Int>
    val local8
      ArrayLiteral : Array<Array<Int>>
        ArrayLiteral : Array<Int>
          IntLiteral 1 : Int
          IntLiteral 2 : Int
        ArrayLiteral : Array<Int>
          IntLiteral 3 : Int
  entry main
";
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
    let expected = "\
Module
  enum Option<T>
    Some(_1: T0)
    None()
  fun print(): Unit <intrinsic rt_print>
  fun println(): Unit <intrinsic rt_println>
  fun first<T>(a: Array<T0>): T0
    return
      Index : T0
        Local a : Array<T0>
        IntLiteral 0 : Int
  fun main(): Unit
    val local0
      Call first<Int> : Int
        ArrayLiteral : Array<Int>
          IntLiteral 1 : Int
          IntLiteral 2 : Int
  entry main
  instance first<Int>
";
    assert_eq!(hir::dump(&module), expected);
}

// --- positive: structural ---

/// The expected type reaches literals in argument position (including
/// the empty literal) and at `return`.
#[test]
fn literal_inference_in_argument_and_return_positions() {
    let file = file(vec![
        fun_sig(
            "consume",
            vec![],
            vec![("m", ty_mutable_int_array())],
            None,
            vec![stmt(call("println", vec![field(var("m"), "size")]))],
        ),
        fun_sig(
            "make",
            vec![],
            vec![],
            Some(ty_int_array()),
            vec![ret(Some(array_lit(vec![int_lit(1)])))],
        ),
        fun(
            "main",
            vec![
                stmt(call(
                    "consume",
                    vec![array_lit(vec![int_lit(1), int_lit(2)])],
                )),
                stmt(call("consume", vec![array_lit(vec![])])),
                val("a", call("make", vec![])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("argument-position literals must lower");

    // The argument literals take the parameter's kind, even the empty
    // one.
    let main_body = match &module.functions[module.entry].kind {
        FunctionKind::User(body) => body,
        FunctionKind::Intrinsic(_) => panic!("main is a user function"),
    };
    for statement in &main_body.statements[..2] {
        let hir::StatementKind::Expr(expr) = &statement.kind else {
            panic!("expected an expression statement");
        };
        let hir::ExprKind::Call { args, .. } = &expr.kind else {
            panic!("expected a call");
        };
        assert!(matches!(args[0].kind, hir::ExprKind::ArrayLiteral(_)));
        assert_eq!(hir::type_name(&module, args[0].ty), "MutableArray<Int>");
    }

    // The literal at `return` takes the function's return type.
    let make = module
        .top_level
        .iter()
        .find(|&&id| module.functions[id].name == "make")
        .copied()
        .expect("make is declared");
    let make_body = match &module.functions[make].kind {
        FunctionKind::User(body) => body,
        FunctionKind::Intrinsic(_) => panic!("make is a user function"),
    };
    let hir::StatementKind::Return { value: Some(value) } = &make_body.statements[0].kind else {
        panic!("expected a return with a value");
    };
    assert!(matches!(value.kind, hir::ExprKind::ArrayLiteral(_)));
    assert_eq!(hir::type_name(&module, value.ty), "Array<Int>");
}

/// Value-type elements: structs inline in the literal, and a field of a
/// subscripted element is directly accessible.
#[test]
fn struct_elements_and_field_through_subscript() {
    let file = file(vec![
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
        ),
        fun(
            "main",
            vec![
                val(
                    "ps",
                    array_lit(vec![
                        struct_init("Point", vec![int_lit(1), int_lit(2)]),
                        struct_init("Point", vec![int_lit(3), int_lit(4)]),
                    ]),
                ),
                val("x", field(subscript(var("ps"), int_lit(1)), "x")),
            ],
        ),
    ]);
    let module = lower_user(file).expect("struct element program must lower");
    let body = match &module.functions[module.entry].kind {
        FunctionKind::User(body) => body,
        FunctionKind::Intrinsic(_) => panic!("main is a user function"),
    };
    let locals: Vec<String> = body
        .locals
        .iter()
        .map(|(_, local)| hir::type_name(&module, local.ty))
        .collect();
    assert_eq!(locals, ["Array<Point>", "Int"]);
}

/// Array types are interned: the annotation and the literal share a
/// `TypeId`, and `Array<Int>` / `MutableArray<Int>` stay distinct
/// (invariance, spec 10.4).
#[test]
fn array_types_are_interned() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("a", Some(ty_int_array()), array_lit(vec![int_lit(1)])),
            val_ty("b", Some(ty_int_array()), array_lit(vec![int_lit(2)])),
            val_ty("m", Some(ty_mutable_int_array()), array_lit(vec![])),
        ],
    )]);
    let module = lower_user(file).expect("interning program must lower");
    let body = match &module.functions[module.entry].kind {
        FunctionKind::User(body) => body,
        FunctionKind::Intrinsic(_) => panic!("main is a user function"),
    };
    let locals: Vec<TypeId> = body.locals.iter().map(|(_, local)| local.ty).collect();
    assert_eq!(locals[0], locals[1], "Array<Int> must be interned");
    assert_ne!(locals[0], locals[2], "Array and MutableArray differ");
    // The literal takes the annotated kind (`Expr::ty` is the
    // annotation's interned type).
    let hir::StatementKind::ValDecl { init, .. } = &body.statements[0].kind else {
        panic!("expected a val declaration");
    };
    assert_eq!(init.ty, locals[0]);
}

/// The conversion constructors resolve before user functions of the
/// same name (spec 10.4, milestone5 DESIGN.md 2.2).
#[test]
fn conversion_resolves_before_user_functions() {
    let file = file(vec![
        fun_expr(
            "Array",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            var("x"),
        ),
        fun(
            "main",
            vec![
                val_ty(
                    "m",
                    Some(ty_mutable_int_array()),
                    array_lit(vec![int_lit(1)]),
                ),
                val("b", call("Array", vec![var("m")])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("conversion precedence program must lower");
    let body = match &module.functions[module.entry].kind {
        FunctionKind::User(body) => body,
        FunctionKind::Intrinsic(_) => panic!("main is a user function"),
    };
    let hir::StatementKind::ValDecl { init, .. } = &body.statements[1].kind else {
        panic!("expected a val declaration");
    };
    assert!(
        matches!(init.kind, hir::ExprKind::ArrayClone(_)),
        "`Array(m)` must be the conversion, not the user function"
    );
    assert_eq!(hir::type_name(&module, init.ty), "Array<Int>");
}

// --- negative: literal inference ---

#[test]
fn empty_literal_requires_an_expected_type() {
    // No annotation at all.
    let file = file(vec![fun("main", vec![val("e", array_lit(vec![]))])]);
    let errors = lower_user(file).expect_err("context-free `[]` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "cannot infer the element type of an empty array literal"
    );

    // An expected type that is not an array does not help either.
    let file2 = super::file(vec![fun(
        "main",
        vec![val_ty("e", Some(ty_named("Int")), array_lit(vec![]))],
    )]);
    let errors = lower_user(file2).expect_err("`[]` under `Int` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "cannot infer the element type of an empty array literal"
    );
}

#[test]
fn mixed_element_types_are_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val("x", array_lit(vec![int_lit(1), str_lit("a")]))],
    )]);
    let errors = lower_user(file).expect_err("mixed elements must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "array literal elements must have the same type, found Int and String"
    );
}

#[test]
fn element_must_match_the_expected_element_type() {
    let file = file(vec![fun(
        "main",
        vec![val_ty(
            "m",
            Some(ty_mutable_int_array()),
            array_lit(vec![int_lit(1), str_lit("a")]),
        )],
    )]);
    let errors = lower_user(file).expect_err("element mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "array literal element must be of type Int, found String"
    );
}

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
        "`Array` takes exactly 1 type argument, but 2 were supplied"
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
        "`Array` requires exactly 1 type argument"
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
