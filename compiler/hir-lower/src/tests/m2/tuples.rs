use super::super::*;

// --- positive: tuples ---

#[test]
fn tuples_and_indexing() {
    let file = file(vec![fun(
        "main",
        vec![
            var_("q", tuple_lit(vec![int_lit(1), str_lit("hello")])),
            val("s", index(var("q"), 2)),
            stmt(call("println", vec![var("s")])),
            val("u", unit_lit()),
            val_ty(
                "t",
                Some(ty_tuple(vec![ty_named("Int")])),
                tuple_lit(vec![int_lit(42)]),
            ),
            stmt(call("print", vec![index(var("t"), 1)])),
        ],
    )]);
    let module = lower_user(file).expect("tuple program must lower");
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
      TupleLiteral : (Int, String)
        IntegerLiteral 1 : Int
        StringLiteral "hello" : String
    val local1
      FieldAccess _2 : String
        Local q : (Int, String)
    val local2
      Local s : String
    val local3
      Local $argument.0 : String
    Call println<String> : Unit
      Local $parameter.value : String
    val local4
      UnitLiteral : Unit
    val local5
      TupleLiteral : (Int)
        IntegerLiteral 42 : Int
    val local6
      FieldAccess _1 : Int
        Local t : (Int)
    val local7
      Local $argument.0 : Int
    Call print<Int> : Unit
      Local $parameter.value : Int
  entry main
  instance println<String>
  instance print<Int>
"#;
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn structs_and_tuples_nest() {
    let file = file(vec![
        struct_decl(
            "Wrap",
            vec![("pair", ty_tuple(vec![ty_named("Int"), ty_named("String")]))],
        ),
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
        ),
        fun(
            "main",
            vec![
                val(
                    "w",
                    call("Wrap", vec![tuple_lit(vec![int_lit(1), str_lit("one")])]),
                ),
                val("n", index(field(var("w"), "pair"), 1)),
                val(
                    "p",
                    tuple_lit(vec![
                        call("Point", vec![int_lit(1), int_lit(2)]),
                        int_lit(3),
                    ]),
                ),
                val("q", index(var("p"), 1)),
                val(
                    "same",
                    binary(
                        BinOp::Eq,
                        var("q"),
                        call("Point", vec![int_lit(1), int_lit(2)]),
                    ),
                ),
                val(
                    "same2",
                    binary(
                        BinOp::Eq,
                        tuple_lit(vec![int_lit(1), str_lit("a")]),
                        tuple_lit(vec![int_lit(1), str_lit("a")]),
                    ),
                ),
                stmt(call("println", vec![var("n")])),
                stmt(call("println", vec![var("same")])),
                stmt(call("println", vec![var("same2")])),
            ],
        ),
    ]);
    lower_user(file).expect("nested struct/tuple program must lower");
}
