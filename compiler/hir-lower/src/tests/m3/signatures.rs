use super::super::*;

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
  interface Iterator<T>
    fun next(): Option<T0>
  interface Iterable<T>
    operator fun iterator(): Iterator<T0>
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
  fun double(x: Int): Int
    val local1
      Local x : Int
    val local2
      IntegerLiteral 2 : Int
    val local3
      Local $argument.0 : Int
    return
      IntegerOperation int.mul target=function64 <no-gc> : Int
        Local $receiver : Int
        Local $parameter.other : Int
  fun main(): Unit
    val local0
      IntegerLiteral 21 : Int
    val local1
      Local $argument.0 : Int
    val local2
      Call double : Int
        Local $parameter.x : Int
    val local3
      Local $argument.0 : Int
    Call println<Int> : Unit
      Local $parameter.value : Int
  entry main
  instance println<Int>
"#;
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
  interface Iterator<T>
    fun next(): Option<T0>
  interface Iterable<T>
    operator fun iterator(): Iterator<T0>
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
    Call f : Unit
  fun f(): Unit
    val local0
      StringLiteral "x" : String
    val local1
      Local $argument.0 : String
    Call println<String> : Unit
      Local $parameter.value : String
    return
  entry main
  instance println<String>
"#;
    assert_eq!(hir::dump(&module), expected);
}
