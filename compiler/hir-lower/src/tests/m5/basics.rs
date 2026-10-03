use super::*;

// --- positive: golden dumps ---

/// Literals in every inference mode, subscript read/write, `.size` on
/// both kinds and both conversion directions.
#[test]
fn array_basics_golden() {
    let file = file(vec![fun(
        "main",
        vec![
            // No expectation: elements share a type, result is `Array`.
            val("a", array_lit(vec![int_lit(1), int_lit(2), int_lit(3)])),
            // A `MutableArray` annotation picks the kind.
            val_ty(
                "m",
                Some(ty_mutable_int_array()),
                array_lit(vec![int_lit(4), int_lit(5)]),
            ),
            // An empty literal is legal with an expected type.
            val_ty("e", Some(ty_int_array()), array_lit(vec![])),
            val("first", subscript(var("a"), int_lit(0))),
            val("n", field(var("a"), "size")),
            val("s", field(var("m"), "size")),
            assign_index(var("m"), int_lit(0), int_lit(40)),
            val("b", call("Array", vec![var("m")])),
            val("m2", call("MutableArray", vec![var("a")])),
            // Nested literals infer `Array<Array<Int>>`.
            val(
                "nested",
                array_lit(vec![
                    array_lit(vec![int_lit(1), int_lit(2)]),
                    array_lit(vec![int_lit(3)]),
                ]),
            ),
        ],
    )]);
    let module = lower_user(file).expect("array program must lower");
    let expected = r#"Module
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  open class Exception(message: Option<String>)
    field0 property9: Option<String>
    property9 val message: Option<String> getter9=body(Exception.$get$message) <stored field0 init=parameter9>
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
  fun __scoopThrowInitializationCycle(message: String): Unit
    val local1
      Local message : String
    val local2
      Local $argument.0 : String
    val local3
      VariantConstruct Option.Some<String> : Option<String>
        Local $parameter._1 : String
    val local4
      Local $argument.0 : Option<String>
    throw
      ClassInit IllegalStateException : IllegalStateException
        Local $parameter.message : Option<String>
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
      ArrayLiteral : Array<Int>
        IntegerLiteral 1 : Int
        IntegerLiteral 2 : Int
        IntegerLiteral 3 : Int
    val local1
      ArrayLiteral : MutableArray<Int>
        IntegerLiteral 4 : Int
        IntegerLiteral 5 : Int
    val local2
      ArrayLiteral : Array<Int>
    val local3
      Local a : Array<Int>
    val local4
      IntegerLiteral 0 : Long
    val local5
      Local $argument.0 : Long
    val local6
      Index ImmutableGet : Int
        Local $receiver : Array<Int>
        Local $parameter.index : Long
    val local7
      ArrayLen : Long
        Local a : Array<Int>
    val local8
      ArrayLen : Long
        Local m : MutableArray<Int>
    val local9
      Local m : MutableArray<Int>
    val local10
      IntegerLiteral 0 : Long
    val local11
      IntegerLiteral 40 : Int
    val local12
      Local $argument.0 : Long
    val local13
      Local $argument.1 : Int
    ArraySet MutableSet : Unit
      Local $receiver : MutableArray<Int>
      Local $parameter.index : Long
      Local $parameter.value : Int
    val local14
      Local m : MutableArray<Int>
    val local15
      Local $argument.0 : MutableArray<Int>
    val local16
      ArrayClone : Array<Int>
        Local $parameter.source : MutableArray<Int>
    val local17
      Local a : Array<Int>
    val local18
      Local $argument.0 : Array<Int>
    val local19
      ArrayClone : MutableArray<Int>
        Local $parameter.source : Array<Int>
    val local20
      ArrayLiteral : Array<Array<Int>>
        ArrayLiteral : Array<Int>
          IntegerLiteral 1 : Int
          IntegerLiteral 2 : Int
        ArrayLiteral : Array<Int>
          IntegerLiteral 3 : Int
  output executable main
"#;
    assert_eq!(hir::dump(&module), expected);
}

/// A generic function over array elements: `T` is inferred from the
/// literal's element type through the `Array<T>` parameter.
#[test]
fn generic_function_over_array_elements() {
    let file = file(vec![
        fun_expr(
            "first",
            vec!["T"],
            vec![("a", ty_generic("Array", vec![ty_named("T")]))],
            Some(ty_named("T")),
            subscript(var("a"), int_lit(0)),
        ),
        fun(
            "main",
            vec![val(
                "x",
                call("first", vec![array_lit(vec![int_lit(1), int_lit(2)])]),
            )],
        ),
    ]);
    let module = lower_user(file).expect("generic array program must lower");
    let expected = r#"Module
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  open class Exception(message: Option<String>)
    field0 property9: Option<String>
    property9 val message: Option<String> getter9=body(Exception.$get$message) <stored field0 init=parameter9>
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
  fun __scoopThrowInitializationCycle(message: String): Unit
    val local1
      Local message : String
    val local2
      Local $argument.0 : String
    val local3
      VariantConstruct Option.Some<String> : Option<String>
        Local $parameter._1 : String
    val local4
      Local $argument.0 : Option<String>
    throw
      ClassInit IllegalStateException : IllegalStateException
        Local $parameter.message : Option<String>
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
  fun first<T>(a: Array<T0>): T0
    val local1
      Local a : Array<T0>
    val local2
      IntegerLiteral 0 : Long
    val local3
      Local $argument.0 : Long
    return
      Index ImmutableGet : T0
        Local $receiver : Array<T0>
        Local $parameter.index : Long
  fun main(): Unit
    val local0
      ArrayLiteral : Array<Int>
        IntegerLiteral 1 : Int
        IntegerLiteral 2 : Int
    val local1
      Local $argument.0 : Array<Int>
    val local2
      Call first<Int> : Int
        Local $parameter.a : Array<Int>
  output executable main
  instance first<Int>
"#;
    assert_eq!(hir::dump(&module), expected);
}
