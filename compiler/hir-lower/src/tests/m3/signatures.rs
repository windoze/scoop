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
    let expected = "\
Module
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  open class Exception(message: Option<String>)
  class UnwrapException()
  class ClassCastException()
  class ArithmeticException()
  class IndexOutOfBoundsException()
  class IllegalStateException()
  interface ToString
    fun toString(): String
  interface Hash
    fun hash(): Int
  interface Continuation<in T>
    fun resume(value: T0): Unit
    fun resumeWithException(exception: Throwable): Unit
  interface SuspendTask<out T>
    suspend fun run(): T0
  interface SuspendRegistration<out T>
    fun register(continuation: Continuation<T0>): Unit
  fun coreIntEquals(arg1: Int, arg2: Int): Boolean <extern0 abi=scoop symbol=scoop_rt_int_equals>
  fun coreUIntEquals(arg1: UInt, arg2: UInt): Boolean <extern1 abi=scoop symbol=scoop_rt_uint_equals>
  fun coreBooleanEquals(arg1: Boolean, arg2: Boolean): Boolean <extern2 abi=scoop symbol=scoop_rt_bool_equals>
  fun coreStringEquals(arg1: String, arg2: String): Boolean <extern3 abi=scoop symbol=scoop_rt_string_eq>
  fun coreIntToString(arg1: Int): String <extern4 abi=scoop symbol=scoop_rt_int_to_string>
  fun coreUIntToString(arg1: UInt): String <extern5 abi=scoop symbol=scoop_rt_uint_to_string>
  fun coreBooleanToString(arg1: Boolean): String <extern6 abi=scoop symbol=scoop_rt_bool_to_string>
  fun coreIntHash(arg1: Int): Int <extern7 abi=scoop symbol=scoop_rt_int_hash>
  fun coreUIntHash(arg1: UInt): Int <extern8 abi=scoop symbol=scoop_rt_uint_hash>
  fun coreBooleanHash(arg1: Boolean): Int <extern9 abi=scoop symbol=scoop_rt_bool_hash>
  fun coreStringHash(arg1: String): Int <extern10 abi=scoop symbol=scoop_rt_string_hash>
  fun startCoroutine<T>(): Unit <intrinsic coroutine_start>
  suspend fun suspendCoroutine<T>(): T0 <intrinsic coroutine_suspend>
  fun write(arg1: String): Unit <extern11 abi=scoop symbol=scoop_rt_write>
  fun print<T : ToString>(value: T0): Unit
    Call write : Unit
      MethodCall bound T0 via ToString -> ToString.toString : String
        Local value : T0
    return
  fun println<T : ToString>(value: T0): Unit
    Call write : Unit
      MethodCall bound T0 via ToString -> ToString.toString : String
        Local value : T0
    Call write : Unit
      StringLiteral \"\\n\" : String
  fun double(x: Int): Int
    return
      Binary Mul : Int
        Local x : Int
        IntLiteral 2 : Int
  fun main(): Unit
    Call println<Int> : Unit
      Call double : Int
        IntLiteral 21 : Int
  entry main
  instance println<Int>
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
  open class Exception(message: Option<String>)
  class UnwrapException()
  class ClassCastException()
  class ArithmeticException()
  class IndexOutOfBoundsException()
  class IllegalStateException()
  interface ToString
    fun toString(): String
  interface Hash
    fun hash(): Int
  interface Continuation<in T>
    fun resume(value: T0): Unit
    fun resumeWithException(exception: Throwable): Unit
  interface SuspendTask<out T>
    suspend fun run(): T0
  interface SuspendRegistration<out T>
    fun register(continuation: Continuation<T0>): Unit
  fun coreIntEquals(arg1: Int, arg2: Int): Boolean <extern0 abi=scoop symbol=scoop_rt_int_equals>
  fun coreUIntEquals(arg1: UInt, arg2: UInt): Boolean <extern1 abi=scoop symbol=scoop_rt_uint_equals>
  fun coreBooleanEquals(arg1: Boolean, arg2: Boolean): Boolean <extern2 abi=scoop symbol=scoop_rt_bool_equals>
  fun coreStringEquals(arg1: String, arg2: String): Boolean <extern3 abi=scoop symbol=scoop_rt_string_eq>
  fun coreIntToString(arg1: Int): String <extern4 abi=scoop symbol=scoop_rt_int_to_string>
  fun coreUIntToString(arg1: UInt): String <extern5 abi=scoop symbol=scoop_rt_uint_to_string>
  fun coreBooleanToString(arg1: Boolean): String <extern6 abi=scoop symbol=scoop_rt_bool_to_string>
  fun coreIntHash(arg1: Int): Int <extern7 abi=scoop symbol=scoop_rt_int_hash>
  fun coreUIntHash(arg1: UInt): Int <extern8 abi=scoop symbol=scoop_rt_uint_hash>
  fun coreBooleanHash(arg1: Boolean): Int <extern9 abi=scoop symbol=scoop_rt_bool_hash>
  fun coreStringHash(arg1: String): Int <extern10 abi=scoop symbol=scoop_rt_string_hash>
  fun startCoroutine<T>(): Unit <intrinsic coroutine_start>
  suspend fun suspendCoroutine<T>(): T0 <intrinsic coroutine_suspend>
  fun write(arg1: String): Unit <extern11 abi=scoop symbol=scoop_rt_write>
  fun print<T : ToString>(value: T0): Unit
    Call write : Unit
      MethodCall bound T0 via ToString -> ToString.toString : String
        Local value : T0
    return
  fun println<T : ToString>(value: T0): Unit
    Call write : Unit
      MethodCall bound T0 via ToString -> ToString.toString : String
        Local value : T0
    Call write : Unit
      StringLiteral \"\\n\" : String
  fun main(): Unit
    Call f : Unit
  fun f(): Unit
    Call println<String> : Unit
      StringLiteral \"x\" : String
    return
  entry main
  instance println<String>
";
    assert_eq!(hir::dump(&module), expected);
}
