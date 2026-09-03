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
    val local0
      TupleLiteral : (Int, String)
        IntLiteral 1 : Int
        StringLiteral \"hello\" : String
    val local1
      FieldAccess _2 : String
        Local q : (Int, String)
    Call println<String> : Unit
      Local s : String
    val local2
      UnitLiteral : Unit
    val local3
      TupleLiteral : (Int)
        IntLiteral 42 : Int
    Call print<Int> : Unit
      FieldAccess _1 : Int
        Local t : (Int)
  entry main
  instance println<String>
  instance print<Int>
";
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
