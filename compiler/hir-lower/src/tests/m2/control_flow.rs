use super::super::*;

// --- positive: operators and control flow ---

#[test]
fn var_rebinding_and_control_flow() {
    let file = file(vec![fun(
        "main",
        vec![
            var_("n", int_lit(0)),
            while_stmt(
                binary(BinOp::Lt, var("n"), int_lit(3)),
                vec![assign("n", binary(BinOp::Add, var("n"), int_lit(1)))],
            ),
            if_stmt(
                binary(
                    BinOp::And,
                    binary(BinOp::Eq, var("n"), int_lit(3)),
                    bool_lit(true),
                ),
                vec![stmt(call("println", vec![str_lit("ok")]))],
                Some(vec![stmt(call("println", vec![str_lit("ng")]))]),
            ),
        ],
    )]);
    let module = lower_user(file).expect("control flow program must lower");
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
    val local0
      IntegerLiteral 0 : Int
    while
      condition setup
        val local1
          Local n : Int
        val local2
          IntegerLiteral 3 : Int
        val local3
          Local $argument.0 : Int
      Binary Lt : Boolean
        IntegerOperation int.compare_to target=function69 <no-gc> : Long
          Local $receiver : Int
          Local $parameter.other : Int
        IntegerLiteral 0 : Long
      val local4
        Local n : Int
      val local5
        IntegerLiteral 1 : Int
      val local6
        Local $argument.0 : Int
      assign n
        IntegerOperation int.add target=function64 <no-gc> : Int
          Local $receiver : Int
          Local $parameter.other : Int
    if
      Binary And : Boolean
        IntegerOperation int.equals target=function70 <no-gc> : Boolean
          Local n : Int
          IntegerLiteral 3 : Int
        BoolLiteral true : Boolean
      val local7
        StringLiteral "ok" : String
      val local8
        Local $argument.0 : String
      Call println<String> : Unit
        Local $parameter.value : String
    else
      val local9
        StringLiteral "ng" : String
      val local10
        Local $argument.0 : String
      Call println<String> : Unit
        Local $parameter.value : String
  entry main
  instance println<String>
"#;
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn unary_operators_and_all_printables() {
    let file = file(vec![fun(
        "main",
        vec![
            val("n", unary(UnOp::Neg, int_lit(5))),
            val("b", unary(UnOp::Not, bool_lit(false))),
            val(
                "m",
                binary(
                    BinOp::Sub,
                    binary(BinOp::Mul, var("n"), int_lit(2)),
                    int_lit(1),
                ),
            ),
            val("d", binary(BinOp::Div, var("m"), int_lit(3))),
            val("le", binary(BinOp::Le, var("d"), int_lit(0))),
            val("ge", binary(BinOp::Ge, var("d"), int_lit(0))),
            val("gt", binary(BinOp::Gt, var("d"), int_lit(0))),
            val("ne", binary(BinOp::Ne, var("le"), var("ge"))),
            val("or", binary(BinOp::Or, var("gt"), var("ne"))),
            stmt(call("println", vec![int_lit(42)])),
            stmt(call("println", vec![bool_lit(true)])),
            stmt(call("print", vec![var("or")])),
        ],
    )]);
    lower_user(file).expect("operator program must lower");
}

// --- positive: scopes ---

#[test]
fn inner_scopes_shadow_and_do_not_leak() {
    let file = file(vec![fun(
        "main",
        vec![
            val("x", int_lit(1)),
            if_stmt(
                bool_lit(true),
                vec![
                    val("x", str_lit("inner")),
                    stmt(call("println", vec![var("x")])),
                ],
                None,
            ),
            stmt(call("println", vec![var("x")])),
            block_stmt(vec![
                val("y", int_lit(2)),
                stmt(call("println", vec![var("y")])),
            ]),
        ],
    )]);
    let module = lower_user(file).expect("shadowing program must lower");
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
    val local0
      IntegerLiteral 1 : Int
    if
      BoolLiteral true : Boolean
      val local1
        StringLiteral "inner" : String
      val local2
        Local x : String
      val local3
        Local $argument.0 : String
      Call println<String> : Unit
        Local $parameter.value : String
    val local4
      Local x : Int
    val local5
      Local $argument.0 : Int
    Call println<Int> : Unit
      Local $parameter.value : Int
    val local6
      IntegerLiteral 2 : Int
    val local7
      Local y : Int
    val local8
      Local $argument.0 : Int
    Call println<Int> : Unit
      Local $parameter.value : Int
  entry main
  instance println<String>
  instance println<Int>
"#;
    assert_eq!(hir::dump(&module), expected);
}
