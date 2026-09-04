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
  class UnwrapException()
  class ClassCastException()
  class ArithmeticException()
  class IndexOutOfBoundsException()
  class IllegalStateException()
  interface ToString
    fun toString(): String
  interface Hash
    fun hash(): Int
  interface Continuation<T>
    fun resume(value: T0): Unit
    fun resumeWithException(exception: Throwable): Unit
  interface SuspendTask<T>
    suspend fun run(): T0
  interface SuspendRegistration<T>
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
        IntLiteral 1 : Int
        IntLiteral 2 : Int
        IntLiteral 3 : Int
    val local1
      ArrayLiteral : MutableArray<Int>
        IntLiteral 4 : Int
        IntLiteral 5 : Int
    val local2
      ArrayLiteral : Array<Int>
    val local3
      Local a : Array<Int>
    val local4
      IntLiteral 0 : Int
    val local5
      Local $argument.0 : Int
    val local6
      Index ImmutableGet : Int
        Local $receiver : Array<Int>
        Local $parameter.index : Int
    val local7
      ArrayLen : Int
        Local a : Array<Int>
    val local8
      ArrayLen : Int
        Local m : MutableArray<Int>
    val local9
      Local m : MutableArray<Int>
    val local10
      IntLiteral 0 : Int
    val local11
      IntLiteral 40 : Int
    val local12
      Local $argument.0 : Int
    val local13
      Local $argument.1 : Int
    ArraySet MutableSet : Unit
      Local $receiver : MutableArray<Int>
      Local $parameter.index : Int
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
          IntLiteral 1 : Int
          IntLiteral 2 : Int
        ArrayLiteral : Array<Int>
          IntLiteral 3 : Int
  entry main
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
  class UnwrapException()
  class ClassCastException()
  class ArithmeticException()
  class IndexOutOfBoundsException()
  class IllegalStateException()
  interface ToString
    fun toString(): String
  interface Hash
    fun hash(): Int
  interface Continuation<T>
    fun resume(value: T0): Unit
    fun resumeWithException(exception: Throwable): Unit
  interface SuspendTask<T>
    suspend fun run(): T0
  interface SuspendRegistration<T>
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
      IntLiteral 0 : Int
    val local3
      Local $argument.0 : Int
    return
      Index ImmutableGet : T0
        Local $receiver : Array<T0>
        Local $parameter.index : Int
  fun main(): Unit
    val local0
      ArrayLiteral : Array<Int>
        IntLiteral 1 : Int
        IntLiteral 2 : Int
    val local1
      Local $argument.0 : Array<Int>
    val local2
      Call first<Int> : Int
        Local $parameter.a : Array<Int>
  entry main
  instance first<Int>
"#;
    assert_eq!(hir::dump(&module), expected);
}
