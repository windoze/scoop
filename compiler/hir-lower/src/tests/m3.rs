//! M3 tests: function signatures, generic functions with call-site
//! type-argument inference, `Option<T>` with `Some` / `None` / `?.` /
//! `?:` / `!!` — golden dumps for inference and desugaring, one
//! negative test per diagnostic.

use super::*;

// --- positive: signatures and returns ---

#[test]
fn expression_body_and_parameters() {
    let file = file(vec![
        fun_expr(
            "double",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            binary(BinOp::Mul, var("x"), int_lit(2)),
        ),
        fun(
            "main",
            vec![stmt(call(
                "println",
                vec![call("double", vec![int_lit(21)])],
            ))],
        ),
    ]);
    let module = lower_user(file).expect("expression body must lower");
    let expected = "\
Module
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  fun write(): Unit <intrinsic rt_write>
  fun print(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    return
  fun println(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    Call write : Unit
      StringLiteral \"\\n\" : String
  fun double(x: Int): Int
    return
      Binary Mul : Int
        Local x : Int
        IntLiteral 2 : Int
  fun main(): Unit
    Call println : Unit
      Box : Any
        Call double : Int
          IntLiteral 21 : Int
  entry main
";
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn block_body_with_return_and_parameters() {
    let file = file(vec![
        fun_sig(
            "add",
            vec![],
            vec![("a", ty_named("Int")), ("b", ty_named("Int"))],
            Some(ty_named("Int")),
            vec![ret(Some(binary(BinOp::Add, var("a"), var("b"))))],
        ),
        fun(
            "main",
            vec![stmt(call(
                "println",
                vec![call("add", vec![int_lit(1), int_lit(2)])],
            ))],
        ),
    ]);
    let module = lower_user(file).expect("block body must lower");
    let dump = hir::dump(&module);
    assert!(dump.contains("fun add(a: Int, b: Int): Int"), "{dump}");
    assert!(dump.contains("Call add : Int"), "{dump}");
}

/// `return <unit value>` in a `Unit` function evaluates the value and
/// lowers to a bare `return` (hir: `Return::value` is absent in `Unit`
/// functions).
#[test]
fn return_with_unit_value_is_a_bare_return() {
    let file = file(vec![
        fun("main", vec![stmt(call("f", vec![]))]),
        fun("f", vec![ret(Some(call("println", vec![str_lit("x")])))]),
    ]);
    let module = lower_user(file).expect("Unit return with value must lower");
    let expected = "\
Module
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  fun write(): Unit <intrinsic rt_write>
  fun print(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    return
  fun println(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    Call write : Unit
      StringLiteral \"\\n\" : String
  fun main(): Unit
    Call f : Unit
  fun f(): Unit
    Call println : Unit
      StringLiteral \"x\" : Any
    return
  entry main
";
    assert_eq!(hir::dump(&module), expected);
}

// --- positive: generics ---

#[test]
fn generic_identity_infers_type_arguments() {
    let file = file(vec![
        fun_expr(
            "identity",
            vec!["T"],
            vec![("x", ty_named("T"))],
            Some(ty_named("T")),
            var("x"),
        ),
        fun(
            "main",
            vec![
                stmt(call("println", vec![call("identity", vec![int_lit(42)])])),
                stmt(call("println", vec![call("identity", vec![str_lit("hi")])])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("generic identity must lower");
    let expected = "\
Module
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  fun write(): Unit <intrinsic rt_write>
  fun print(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    return
  fun println(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    Call write : Unit
      StringLiteral \"\\n\" : String
  fun identity<T>(x: T0): T0
    return
      Local x : T0
  fun main(): Unit
    Call println : Unit
      Box : Any
        Call identity<Int> : Int
          IntLiteral 42 : Int
    Call println : Unit
      Call identity<String> : Any
        StringLiteral \"hi\" : String
  entry main
  instance identity<Int>
  instance identity<String>
";
    assert_eq!(hir::dump(&module), expected);
}

/// Multiple type parameters bind independently; tuple return types
/// substitute recursively.
#[test]
fn multiple_type_parameters() {
    let file = file(vec![
        fun_expr(
            "pair",
            vec!["T", "U"],
            vec![("a", ty_named("T")), ("b", ty_named("U"))],
            Some(ty_tuple(vec![ty_named("T"), ty_named("U")])),
            tuple_lit(vec![var("a"), var("b")]),
        ),
        fun(
            "main",
            vec![val("p", call("pair", vec![int_lit(1), str_lit("s")]))],
        ),
    ]);
    let module = lower_user(file).expect("two type parameters must lower");
    let dump = hir::dump(&module);
    assert!(
        dump.contains("fun pair<T, U>(a: T0, b: T1): (T0, T1)"),
        "{dump}"
    );
    assert!(
        dump.contains("Call pair<Int, String> : (Int, String)"),
        "{dump}"
    );
    assert!(dump.contains("instance pair<Int, String>"), "{dump}");
}

/// `==` / `!=` are allowed on type parameters (both sides same type).
#[test]
fn generic_equality_is_allowed() {
    let file = file(vec![
        fun_expr(
            "eq",
            vec!["T"],
            vec![("a", ty_named("T")), ("b", ty_named("T"))],
            Some(ty_named("Boolean")),
            binary(BinOp::Eq, var("a"), var("b")),
        ),
        fun(
            "main",
            vec![stmt(call(
                "println",
                vec![call("eq", vec![int_lit(1), int_lit(2)])],
            ))],
        ),
    ]);
    lower_user(file).expect("generic equality must lower");
}

/// A generic call inside a generic body records an instantiation
/// request whose type arguments may still mention `Type::Param`;
/// mir-lower concretizes them when the requesting instance is
/// materialized.
#[test]
fn nested_generic_calls_record_param_instantiations() {
    let file = file(vec![
        fun_expr(
            "identity",
            vec!["T"],
            vec![("x", ty_named("T"))],
            Some(ty_named("T")),
            var("x"),
        ),
        fun_sig(
            "twice",
            vec!["U"],
            vec![("v", ty_named("U"))],
            Some(ty_named("U")),
            vec![ret(Some(call(
                "identity",
                vec![call("identity", vec![var("v")])],
            )))],
        ),
        fun(
            "main",
            vec![stmt(call(
                "println",
                vec![call("twice", vec![int_lit(21)])],
            ))],
        ),
    ]);
    let module = lower_user(file).expect("nested generic calls must lower");
    let expected = "\
Module
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  fun write(): Unit <intrinsic rt_write>
  fun print(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    return
  fun println(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    Call write : Unit
      StringLiteral \"\\n\" : String
  fun identity<T>(x: T0): T0
    return
      Local x : T0
  fun twice<U>(v: T0): T0
    return
      Call identity<T0> : T0
        Call identity<T0> : T0
          Local v : T0
  fun main(): Unit
    Call println : Unit
      Box : Any
        Call twice<Int> : Int
          IntLiteral 21 : Int
  entry main
  instance identity<T0>
  instance twice<Int>
";
    assert_eq!(hir::dump(&module), expected);
}

/// `Option<T>` in a parameter type binds recursively against an
/// `Option<Int>` argument; the elvis desugaring works over
/// `Option<T0>` inside the generic body.
#[test]
fn generic_option_roundtrip() {
    let file = file(vec![
        fun_sig(
            "unwrapOr",
            vec!["T"],
            vec![
                ("o", ty_nullable(ty_named("T"))),
                ("fallback", ty_named("T")),
            ],
            Some(ty_named("T")),
            vec![ret(Some(elvis(var("o"), var("fallback"))))],
        ),
        fun(
            "main",
            vec![
                val_ty("a", Some(ty_nullable(ty_named("Int"))), some(int_lit(41))),
                stmt(call(
                    "println",
                    vec![call("unwrapOr", vec![var("a"), int_lit(0)])],
                )),
            ],
        ),
    ]);
    let module = lower_user(file).expect("generic Option function must lower");
    let expected = "\
Module
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  fun write(): Unit <intrinsic rt_write>
  fun print(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    return
  fun println(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    Call write : Unit
      StringLiteral \"\\n\" : String
  fun unwrapOr<T>(o: Option<T0>, fallback: T0): T0
    val local2
      Local o : Option<T0>
    if
      IsSome : Boolean
        Local $opt.0 : Option<T0>
      val local3
        Unwrap trap=false : T0
          Local $opt.0 : Option<T0>
    else
      val local3
        Local fallback : T0
    return
      Local $res.1 : T0
  fun main(): Unit
    val local0
      VariantConstruct Option.Some<Int> : Option<Int>
        IntLiteral 41 : Int
    Call println : Unit
      Box : Any
        Call unwrapOr<Int> : Int
          Local a : Option<Int>
          IntLiteral 0 : Int
  entry main
  instance unwrapOr<Int>
";
    assert_eq!(hir::dump(&module), expected);
}

// --- positive: Option and nullables ---

#[test]
fn some_none_and_nullable_annotations() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("a", Some(ty_nullable(ty_named("Int"))), some(int_lit(41))),
            val_ty("b", Some(ty_nullable(ty_named("Int"))), none()),
            // `T??` is `Option<Option<T>>` and does not collapse; the
            // expected type lets `Some(None)` type the inner `None`.
            val_ty(
                "c",
                Some(ty_nullable(ty_nullable(ty_named("Int")))),
                some(none()),
            ),
        ],
    )]);
    let module = lower_user(file).expect("Option constructors must lower");
    let expected = "\
Module
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  fun write(): Unit <intrinsic rt_write>
  fun print(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    return
  fun println(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    Call write : Unit
      StringLiteral \"\\n\" : String
  fun main(): Unit
    val local0
      VariantConstruct Option.Some<Int> : Option<Int>
        IntLiteral 41 : Int
    val local1
      VariantConstruct Option.None<Int> : Option<Int>
    val local2
      VariantConstruct Option.Some<Option<Int>> : Option<Option<Int>>
        VariantConstruct Option.None<Int> : Option<Int>
  entry main
";
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn safe_field_access_desugars_to_hidden_locals() {
    let file = file(vec![
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
        ),
        fun(
            "main",
            vec![
                val_ty(
                    "p",
                    Some(ty_nullable(ty_named("Point"))),
                    some(call("Point", vec![int_lit(1), int_lit(2)])),
                ),
                val("x", safe_field(var("p"), "x")),
            ],
        ),
    ]);
    let module = lower_user(file).expect("safe field access must lower");
    let expected = "\
Module
  struct Point
    field x: Int
    field y: Int
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  fun write(): Unit <intrinsic rt_write>
  fun print(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    return
  fun println(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    Call write : Unit
      StringLiteral \"\\n\" : String
  fun main(): Unit
    val local0
      VariantConstruct Option.Some<Point> : Option<Point>
        StructInit Point : Point
          IntLiteral 1 : Int
          IntLiteral 2 : Int
    val local1
      Local p : Option<Point>
    if
      IsSome : Boolean
        Local $opt.0 : Option<Point>
      val local2
        SomeWrap : Option<Int>
          FieldAccess field 0 : Int
            Unwrap trap=false : Point
              Local $opt.0 : Option<Point>
    else
      val local2
        NoneLiteral : Option<Int>
    val local3
      Local $res.1 : Option<Int>
  entry main
";
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn elvis_desugars_to_hidden_locals() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("a", Some(ty_nullable(ty_named("Int"))), some(int_lit(41))),
            val("b", elvis(var("a"), int_lit(0))),
        ],
    )]);
    let module = lower_user(file).expect("elvis must lower");
    let expected = "\
Module
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  fun write(): Unit <intrinsic rt_write>
  fun print(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    return
  fun println(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    Call write : Unit
      StringLiteral \"\\n\" : String
  fun main(): Unit
    val local0
      VariantConstruct Option.Some<Int> : Option<Int>
        IntLiteral 41 : Int
    val local1
      Local a : Option<Int>
    if
      IsSome : Boolean
        Local $opt.0 : Option<Int>
      val local2
        Unwrap trap=false : Int
          Local $opt.0 : Option<Int>
    else
      val local2
        IntLiteral 0 : Int
    val local3
      Local $res.1 : Int
  entry main
";
    assert_eq!(hir::dump(&module), expected);
}

/// `w?.p?.x` chains: `w?.p` is an `Option<Point>`, whose `?.x` is an
/// `Option<Int>` — the desugarings nest through the sink.
#[test]
fn chained_safe_field_access() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        struct_decl("Wrap", vec![("p", ty_named("Point"))]),
        fun(
            "main",
            vec![
                val_ty(
                    "w",
                    Some(ty_nullable(ty_named("Wrap"))),
                    some(call("Wrap", vec![call("Point", vec![int_lit(1)])])),
                ),
                val("x", safe_field(safe_field(var("w"), "p"), "x")),
            ],
        ),
    ]);
    let module = lower_user(file).expect("chained safe access must lower");
    let dump = hir::dump(&module);
    // w?.p : Option<Point>, then ?.x : Option<Int>. Hidden locals are
    // numbered per body: w, $opt.0, $res.1, $opt.2, $res.3, x.
    assert!(
        dump.contains("val local1\n      Local w : Option<Wrap>"),
        "{dump}"
    );
    assert!(
        dump.contains("val local2\n        SomeWrap : Option<Point>"),
        "{dump}"
    );
    assert!(
        dump.contains("val local3\n      Local $res.1 : Option<Point>"),
        "{dump}"
    );
    assert!(
        dump.contains("val local5\n      Local $res.3 : Option<Int>"),
        "{dump}"
    );
}

#[test]
fn null_assert_unwraps_with_trap() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("a", Some(ty_nullable(ty_named("Int"))), some(int_lit(41))),
            val("x", null_assert(var("a"))),
            stmt(call("println", vec![var("x")])),
        ],
    )]);
    let module = lower_user(file).expect("null assert must lower");
    let dump = hir::dump(&module);
    assert!(
        dump.contains("val local1\n      Unwrap trap=true : Int\n        Local a : Option<Int>"),
        "{dump}"
    );
}

/// `x == None` and `None == x`: the literal takes the other side's
/// type from the expected-type hint.
#[test]
fn none_equality_comparison() {
    for comparison in [
        binary(BinOp::Eq, var("a"), none()),
        binary(BinOp::Ne, none(), var("a")),
    ] {
        let file = file(vec![fun(
            "main",
            vec![
                val_ty("a", Some(ty_nullable(ty_named("Int"))), none()),
                if_stmt(
                    comparison,
                    vec![stmt(call("println", vec![str_lit("empty")]))],
                    None,
                ),
            ],
        )]);
        let module = lower_user(file).expect("`== None` must lower");
        let dump = hir::dump(&module);
        assert!(
            dump.contains("VariantConstruct Option.None<Int> : Option<Int>"),
            "{dump}"
        );
    }
}

// --- negative: signatures and returns ---

#[test]
fn return_type_mismatch_is_an_error() {
    let file = file(vec![
        fun_sig(
            "f",
            vec![],
            vec![],
            Some(ty_named("Int")),
            vec![ret(Some(str_lit("s")))],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("return mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`return` value of `f` must be of type Int, found String"
    );
}

#[test]
fn non_unit_function_must_end_with_return() {
    for body in [
        vec![],
        vec![stmt(call("println", vec![str_lit("x")]))],
        vec![if_stmt(bool_lit(true), vec![ret(Some(int_lit(1)))], None)],
    ] {
        let file = file(vec![
            fun_sig("f", vec![], vec![], Some(ty_named("Int")), body),
            fun("main", vec![]),
        ]);
        let errors = lower_user(file).expect_err("missing return must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "non-Unit function `f` must end with a return statement"
        );
    }
}

#[test]
fn bare_return_in_non_unit_function_is_an_error() {
    let file = file(vec![
        fun_sig("f", vec![], vec![], Some(ty_named("Int")), vec![ret(None)]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("bare return must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`return` without a value in function `f` returning Int"
    );
}

#[test]
fn return_value_must_match_unit() {
    let file = file(vec![
        fun("main", vec![]),
        fun("f", vec![ret(Some(int_lit(1)))]),
    ]);
    let errors = lower_user(file).expect_err("non-Unit value in Unit function must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`return` value of `f` must be of type Unit, found Int"
    );
}

#[test]
fn expression_body_type_mismatch_is_an_error() {
    let file = file(vec![
        fun_expr("f", vec![], vec![], Some(ty_named("Int")), str_lit("s")),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("body mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "body of `f` must be of type Int, found String"
    );
}

#[test]
fn duplicate_type_parameter_is_an_error() {
    let file = file(vec![
        fun_sig("f", vec!["T", "T"], vec![], None, vec![]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("duplicate type parameter must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate type parameter `T`");
}

#[test]
fn duplicate_parameter_is_an_error() {
    let file = file(vec![
        fun_sig(
            "f",
            vec![],
            vec![("x", ty_named("Int")), ("x", ty_named("Int"))],
            None,
            vec![],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("duplicate parameter must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate parameter `x`");
}

#[test]
fn generic_main_is_an_error() {
    let file = file(vec![fun_sig("main", vec!["T"], vec![], None, vec![])]);
    let errors = lower_user(file).expect_err("generic main must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`main` must not be generic");
}

// --- negative: generics ---

#[test]
fn unbound_type_argument_is_an_error() {
    let file = file(vec![
        fun_sig(
            "f",
            vec!["T"],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            vec![ret(Some(var("x")))],
        ),
        fun(
            "main",
            vec![stmt(call("println", vec![call("f", vec![int_lit(1)])]))],
        ),
    ]);
    let errors = lower_user(file).expect_err("unbound type argument must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "cannot infer type argument `T` for `f`");
}

#[test]
fn conflicting_type_arguments_are_an_error() {
    let file = file(vec![
        fun_expr(
            "f",
            vec!["T"],
            vec![("a", ty_named("T")), ("b", ty_named("T"))],
            Some(ty_named("T")),
            var("a"),
        ),
        fun(
            "main",
            vec![val("x", call("f", vec![int_lit(1), str_lit("s")]))],
        ),
    ]);
    let errors = lower_user(file).expect_err("conflicting bindings must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "conflicting types for `T`: Int and String"
    );
}

#[test]
fn argument_type_mismatch_is_an_error() {
    let file = file(vec![
        fun_expr(
            "f",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            var("x"),
        ),
        fun("main", vec![stmt(call("f", vec![str_lit("s")]))]),
    ]);
    let errors = lower_user(file).expect_err("argument mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "argument for parameter `x` of `f` must be of type Int, found String"
    );
}

/// Type parameters support no concrete operations beyond `==` / `!=`:
/// `T` is unconstrained at the definition site.
#[test]
fn arithmetic_on_a_type_parameter_is_an_error() {
    let file = file(vec![
        fun_expr(
            "f",
            vec!["T"],
            vec![("x", ty_named("T"))],
            Some(ty_named("T")),
            binary(BinOp::Add, var("x"), int_lit(1)),
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("arithmetic on `T` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "operator `+` requires Int operands, found T and Int"
    );
}

// --- negative: Option ---

#[test]
fn null_assert_on_non_option_is_an_error() {
    let file = file(vec![fun("main", vec![val("x", null_assert(int_lit(1)))])]);
    let errors = lower_user(file).expect_err("`!!` on Int must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`!!` requires an Option operand, found Int"
    );
}

#[test]
fn safe_field_access_on_non_option_is_an_error() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("p", call("Point", vec![int_lit(1)])),
                val("x", safe_field(var("p"), "x")),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("`?.` on Point must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`?.` requires an Option receiver, found Point"
    );
}

#[test]
fn elvis_on_non_option_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val("x", elvis(int_lit(1), int_lit(0)))],
    )]);
    let errors = lower_user(file).expect_err("`?:` on Int must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`?:` requires an Option left-hand side, found Int"
    );
}

#[test]
fn elvis_right_hand_side_must_match() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("a", Some(ty_nullable(ty_named("Int"))), some(int_lit(1))),
            val("x", elvis(var("a"), str_lit("s"))),
        ],
    )]);
    let errors = lower_user(file).expect_err("elvis rhs mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "right-hand side of `?:` must be of type Int, found String"
    );
}

#[test]
fn none_without_expected_type_is_an_error() {
    for init in [none(), some(none())] {
        let file = file(vec![fun("main", vec![val("x", init)])]);
        let errors = lower_user(file).expect_err("untyped `None` must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "cannot infer the type of `None`");
    }
}

#[test]
fn some_arity_is_an_error() {
    let file = file(vec![fun("main", vec![val("x", call("Some", vec![]))])]);
    let errors = lower_user(file).expect_err("`Some` arity must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "variant `Some` of `Option` takes exactly 1 argument, but 0 were supplied"
    );
}

/// A while condition is re-evaluated per iteration, but the `?.`/`?:`
/// desugaring statements would execute once before the loop — rejected
/// instead of silently changing evaluation semantics.
#[test]
fn desugaring_in_while_condition_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty(
                "a",
                Some(ty_nullable(ty_named("Boolean"))),
                some(bool_lit(true)),
            ),
            while_stmt(elvis(var("a"), bool_lit(false)), vec![]),
        ],
    )]);
    let errors = lower_user(file).expect_err("elvis in while condition must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`?.` and `?:` are not allowed in a while condition"
    );
}
