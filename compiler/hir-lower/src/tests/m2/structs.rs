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
    assert!(matches!(
        module.types[int_type(&module)],
        Type::Integer(hir::IntegerKind::SIGNED_32)
    ));
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
      FieldAccess field 0 : Int
        Local p : Point
    val local6
      Local x : Int
    val local7
      Local $argument.0 : Int
    Call println<Int> : Unit
      Local $parameter.value : Int
  output executable main
  instance println<Int>
"#;
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
