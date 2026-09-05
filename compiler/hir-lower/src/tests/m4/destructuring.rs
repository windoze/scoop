use super::*;

// --- positive: destructuring declarations ---

#[test]
fn destructuring_declarations() {
    let file = file(vec![
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
        ),
        fun(
            "main",
            vec![
                // Tuple, with `..` skipping the middle elements.
                val(
                    "t",
                    tuple_lit(vec![int_lit(1), int_lit(2), int_lit(3), int_lit(4)]),
                ),
                val_pat(
                    false,
                    ast::Pattern::Tuple {
                        elements: vec![
                            pat_bind_at("a", Span::new(10, 11)),
                            pat_bind_at("b", Span::new(30, 31)),
                        ],
                        rest: Some(Span::new(20, 22)),
                        span: sp(),
                    },
                    None,
                    var("t"),
                ),
                // Struct field pattern with rename and `..`.
                val("p", call("Point", vec![int_lit(1), int_lit(2)])),
                val_pat(
                    false,
                    pat_named(&["Point"], vec![("x", Some("px"))], Some(sp())),
                    None,
                    var("p"),
                ),
                // Struct positional pattern, `var` bindings are mutable.
                var_pat2(pat_tuple(vec![pat_bind("qx"), pat_wild()], None), var("p")),
                // Nested tuple-in-tuple.
                val(
                    "nested",
                    tuple_lit(vec![tuple_lit(vec![int_lit(1), int_lit(2)]), str_lit("s")]),
                ),
                val_pat(
                    false,
                    pat_tuple(
                        vec![
                            pat_tuple(vec![pat_bind("m"), pat_bind("n")], None),
                            pat_wild(),
                        ],
                        None,
                    ),
                    None,
                    var("nested"),
                ),
                stmt(call("println", vec![var("a")])),
                stmt(call("println", vec![var("b")])),
                stmt(call("println", vec![var("px")])),
                stmt(call("println", vec![var("m")])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("destructuring must lower");
    let expected = r#"Module
  struct Point
    field0 x: Int
    field1 y: Int
    property9 val x: Int getter9=storage <stored struct15-field0>
    property10 val y: Int getter10=storage <stored struct15-field1>
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  open class Exception(message: Option<String>)
    field0 property11: Option<String>
    property11 val message: Option<String> getter11=storage <stored field0 init=parameter11>
  class UnwrapException()
  class ClassCastException()
  class ArithmeticException()
  class IndexOutOfBoundsException()
  class IllegalStateException(message: Option<String>)
  interface ToString
    fun toString(): String
  interface Hash
    fun hash(): Long
  interface Continuation<T>
    fun resume(value: T0): Unit
    fun resumeWithException(exception: Throwable): Unit
  interface SuspendTask<T>
    suspend fun run(): T0
  interface SuspendRegistration<T>
    fun register(continuation: Continuation<T0>): Unit
  fun coreBooleanEquals(arg1: Boolean, arg2: Boolean): Boolean <extern0 abi=scoop symbol=scoop_rt_bool_equals>
  fun coreStringEquals(arg1: String, arg2: String): Boolean <extern1 abi=scoop symbol=scoop_rt_string_eq>
  fun coreLongToString(arg1: Long): String <extern2 abi=scoop symbol=scoop_rt_long_to_string>
  fun coreULongToString(arg1: ULong): String <extern3 abi=scoop symbol=scoop_rt_ulong_to_string>
  fun coreBooleanToString(arg1: Boolean): String <extern4 abi=scoop symbol=scoop_rt_bool_to_string>
  fun coreLongHash(arg1: Long): Long <extern5 abi=scoop symbol=scoop_rt_long_hash>
  fun coreULongHash(arg1: ULong): Long <extern6 abi=scoop symbol=scoop_rt_ulong_hash>
  fun coreBooleanHash(arg1: Boolean): Long <extern7 abi=scoop symbol=scoop_rt_bool_hash>
  fun coreStringHash(arg1: String): Long <extern8 abi=scoop symbol=scoop_rt_string_hash>
  fun startCoroutine<T>(): Unit <intrinsic coroutine_start>
  suspend fun suspendCoroutine<T>(): T0 <intrinsic coroutine_suspend>
  fun write(arg1: String): Unit <extern9 abi=scoop symbol=scoop_rt_write>
  fun print<T : ToString>(value: T0): Unit
    val local1
      Local value : T0
    val local2
      MethodCall bound T0 via ToString -> ToString.toString : String
        Local $receiver : T0
    val local3
      Local $argument.0 : String
    Call write : Unit
      Local $parameter.message : String
    return
  fun println<T : ToString>(value: T0): Unit
    val local1
      Local value : T0
    val local2
      MethodCall bound T0 via ToString -> ToString.toString : String
        Local $receiver : T0
    val local3
      Local $argument.0 : String
    Call write : Unit
      Local $parameter.message : String
    val local4
      StringLiteral "\n" : String
    val local5
      Local $argument.0 : String
    Call write : Unit
      Local $parameter.message : String
  fun main(): Unit
    val local0
      TupleLiteral : (Int, Int, Int, Int)
        IntegerLiteral 1 : Int
        IntegerLiteral 2 : Int
        IntegerLiteral 3 : Int
        IntegerLiteral 4 : Int
    val (local1, _, _, local2)
      Local t : (Int, Int, Int, Int)
    val local3
      IntegerLiteral 1 : Int
    val local4
      IntegerLiteral 2 : Int
    val local5
      Local $argument.0 : Int
    val local6
      Local $argument.1 : Int
    val local7
      StructInit Point : Point
        Local $parameter.x : Int
        Local $parameter.y : Int
    val struct(0: local8)
      Local p : Point
    val struct(0: local9, 1: _)
      Local p : Point
    val local10
      TupleLiteral : ((Int, Int), String)
        TupleLiteral : (Int, Int)
          IntegerLiteral 1 : Int
          IntegerLiteral 2 : Int
        StringLiteral "s" : String
    val ((local11, local12), _)
      Local nested : ((Int, Int), String)
    val local13
      Local a : Int
    val local14
      Local $argument.0 : Int
    Call println<Int> : Unit
      Local $parameter.value : Int
    val local15
      Local b : Int
    val local16
      Local $argument.0 : Int
    Call println<Int> : Unit
      Local $parameter.value : Int
    val local17
      Local px : Int
    val local18
      Local $argument.0 : Int
    Call println<Int> : Unit
      Local $parameter.value : Int
    val local19
      Local m : Int
    val local20
      Local $argument.0 : Int
    Call println<Int> : Unit
      Local $parameter.value : Int
  entry main
  instance println<Int>
"#;
    assert_eq!(hir::dump(&module), expected);
}

/// `var` destructuring: bindings are mutable.
fn var_pat2(target: ast::Pattern, init: Expr) -> Statement {
    val_pat(true, target, None, init)
}

// --- negative: destructuring declarations ---

#[test]
fn refutable_patterns_in_val_are_an_error() {
    for (target, init) in [
        (pat_pos(&["Some"], vec![pat_bind("x")], None), var("o")),
        (pat_lit(int_lit(0)), var("o")),
        // Nested refutable pattern inside an irrefutable one.
        (
            pat_tuple(
                vec![pat_pos(&["Some"], vec![pat_bind("x")], None), pat_wild()],
                None,
            ),
            tuple_lit(vec![var("o"), int_lit(2)]),
        ),
    ] {
        let file = file(vec![fun(
            "main",
            vec![
                val_ty("o", Some(ty_nullable(ty_named("Int"))), some(int_lit(1))),
                val_pat(false, target, None, init),
            ],
        )]);
        let errors = lower_user(file).expect_err("refutable pattern in val must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "refutable patterns are only allowed in `when`"
        );
    }
}

/// A bare `None` in a `val` target is a (refutable) unit variant
/// pattern, not a binding.
#[test]
fn bare_unit_variant_in_val_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("o", Some(ty_nullable(ty_named("Int"))), none()),
            val_pat(false, pat_bind("None"), None, var("o")),
        ],
    )]);
    let errors = lower_user(file).expect_err("`val None` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "refutable patterns are only allowed in `when`"
    );
}

#[test]
fn destructuring_type_mismatch_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val_pat(
            false,
            pat_tuple(vec![pat_bind("x"), pat_bind("y")], None),
            None,
            tuple_lit(vec![int_lit(1)]),
        )],
    )]);
    let errors = lower_user(file).expect_err("pattern/type mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "pattern has 2 element(s), but tuple of type (Int) has 1"
    );
}

#[test]
fn literal_pattern_type_mismatch_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val("t", tuple_lit(vec![int_lit(1), str_lit("s")])),
            when_stmt(
                var("t"),
                vec![
                    arm(
                        pat_tuple(vec![pat_lit(str_lit("x")), pat_bind("s")], None),
                        None,
                        vec![],
                    ),
                    arm(pat_wild(), None, vec![]),
                ],
                None,
            ),
        ],
    )]);
    let errors = lower_user(file).expect_err("literal mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "literal pattern of type String cannot match Int"
    );
}

#[test]
fn non_literal_literal_pattern_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val("t", tuple_lit(vec![int_lit(1)])),
            when_stmt(
                var("t"),
                vec![
                    arm(
                        pat_tuple(vec![pat_lit(call("f", vec![]))], None),
                        None,
                        vec![],
                    ),
                    arm(pat_wild(), None, vec![]),
                ],
                None,
            ),
        ],
    )]);
    let errors = lower_user(file).expect_err("non-literal pattern must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "expected a literal pattern");
}

#[test]
fn duplicate_binding_in_pattern_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val("t", tuple_lit(vec![int_lit(1), int_lit(2)])),
            when_stmt(
                var("t"),
                vec![arm(
                    pat_tuple(vec![pat_bind("x"), pat_bind("x")], None),
                    None,
                    vec![],
                )],
                None,
            ),
        ],
    )]);
    let errors = lower_user(file).expect_err("duplicate binding must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`x` is already declared in this scope");
}
