use super::super::*;

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
    let expected = r#"Module
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  open class Exception(message: Option<String>)
    field0 property9: Option<String>
    property9 val message: Option<String> getter9=storage <stored field0 init=parameter9>
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
      IntegerLiteral 41 : Int
    val local1
      Local $argument.0 : Int
    val local2
      VariantConstruct Option.Some<Int> : Option<Int>
        Local $parameter._1 : Int
    val local3
      VariantConstruct Option.None<Int> : Option<Int>
    val local4
      VariantConstruct Option.None<Int> : Option<Int>
    val local5
      Local $argument.0 : Option<Int>
    val local6
      VariantConstruct Option.Some<Option<Int>> : Option<Option<Int>>
        Local $parameter._1 : Option<Int>
  entry main
"#;
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
      IntegerLiteral 1 : Int
    val local1
      IntegerLiteral 2 : Int
    val local2
      Local $argument.0 : Int
    val local3
      Local $argument.1 : Int
    val local4
      StructInit Point : Point
        Local $parameter.x : Int
        Local $parameter.y : Int
    val local5
      Local $argument.0 : Point
    val local6
      VariantConstruct Option.Some<Point> : Option<Point>
        Local $parameter._1 : Point
    val local7
      Local p : Option<Point>
    if
      IsSome : Boolean
        Local $opt.0 : Option<Point>
      val local8
        SomeWrap : Option<Int>
          FieldAccess field 0 : Int
            Unwrap trap=false : Point
              Local $opt.0 : Option<Point>
    else
      val local8
        NoneLiteral : Option<Int>
    val local9
      Local $res.1 : Option<Int>
  entry main
"#;
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
    let expected = r#"Module
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  open class Exception(message: Option<String>)
    field0 property9: Option<String>
    property9 val message: Option<String> getter9=storage <stored field0 init=parameter9>
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
      IntegerLiteral 41 : Int
    val local1
      Local $argument.0 : Int
    val local2
      VariantConstruct Option.Some<Int> : Option<Int>
        Local $parameter._1 : Int
    val local3
      Local a : Option<Int>
    if
      IsSome : Boolean
        Local $opt.0 : Option<Int>
      val local4
        Unwrap trap=false : Int
          Local $opt.0 : Option<Int>
    else
      val local4
        IntegerLiteral 0 : Int
    val local5
      Local $res.1 : Int
  entry main
"#;
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
    // w?.p : Option<Point>, then ?.x : Option<Int>. Call materialization may
    // add locals before these desugaring sinks, so their typed names are the
    // stable part of this contract.
    assert!(dump.contains("Local w : Option<Wrap>"), "{dump}");
    assert!(dump.contains("SomeWrap : Option<Point>"), "{dump}");
    assert!(dump.contains("Local $res.1 : Option<Point>"), "{dump}");
    assert!(dump.contains("Local $res.3 : Option<Int>"), "{dump}");
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
        dump.contains("Unwrap trap=true : Int\n        Local a : Option<Int>"),
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
