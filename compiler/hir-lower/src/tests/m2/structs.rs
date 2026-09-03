use super::super::*;

// --- positive: structs ---

#[test]
fn struct_construction_and_field_access() {
    let file = file(vec![
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
        ),
        fun(
            "main",
            vec![
                val("p", call("Point", vec![int_lit(1), int_lit(2)])),
                val("x", field(var("p"), "x")),
                stmt(call("println", vec![var("x")])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("struct program must lower");

    // The struct type is allocated right after the well-known types.
    assert!(matches!(module.types[module.int], Type::Int));
    let expected = "\
Module
  struct Point
    field x: Int
    field y: Int
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
      StructInit Point : Point
        IntLiteral 1 : Int
        IntLiteral 2 : Int
    val local1
      FieldAccess field 0 : Int
        Local p : Point
    Call println<Int> : Unit
      Local x : Int
  entry main
  instance println<Int>
";
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn struct_init_node_also_constructs() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![val("p", struct_init("Point", vec![int_lit(1)]))],
        ),
    ]);
    let module = lower_user(file).expect("StructInit node must lower");
    let dump = hir::dump(&module);
    assert!(dump.contains("StructInit Point : Point"), "{dump}");
}

/// A struct and a function may share a name (separate namespaces). In
/// `Name(...)` call position the struct namespace wins: the expression
/// is a construction, not a call.
#[test]
fn struct_shadows_function_in_call_position() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun("Point", vec![]),
        fun("main", vec![val("p", call("Point", vec![int_lit(5)]))]),
    ]);
    let module = lower_user(file).expect("struct/function name sharing must lower");
    let dump = hir::dump(&module);
    assert!(dump.contains("StructInit Point : Point"), "{dump}");
    assert!(dump.contains("fun Point"), "{dump}");
}

#[test]
fn struct_fields_may_reference_later_structs() {
    let file = file(vec![
        struct_decl("A", vec![("b", ty_named("B"))]),
        struct_decl("B", vec![("v", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("a", call("A", vec![call("B", vec![int_lit(1)])])),
                val("v", field(field(var("a"), "b"), "v")),
                stmt(call("println", vec![var("v")])),
            ],
        ),
    ]);
    lower_user(file).expect("forward struct references must lower");
}
