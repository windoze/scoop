use super::*;

// --- GC intrinsics: the happy path (golden dump) ---

#[test]
fn gc_intrinsics_golden() {
    let file = file(vec![fun(
        "main",
        vec![unsafe_block(vec![
            val("s", str_lit("hello")),
            val("h", call("pin", vec![var("s")])),
            val("s2", call("unpin", vec![var("h")])),
            val("g", call("getGcHandle", vec![var("s")])),
            val("s3", call("releaseGcHandle", vec![var("g")])),
            stmt(call("gcCollect", vec![])),
            val_ty("n", Some(ty_named("ULong")), call("gcStats", vec![])),
        ])],
    )]);
    let module = lower_user_with_gc(file).expect("the GC program must lower");
    assert_eq!(local_ty(&module, "s2"), "String");
    assert_eq!(local_ty(&module, "s3"), "String");
    assert_eq!(local_ty(&module, "n"), "ULong");
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
  fun pin<T : ref>(v: T0): PinnedPtr<T0> <unsafe>
    val local1
      Local v : T0
    val local2
      Local $argument.0 : T0
    val local3
      Call _pin<T0> : ULong
        Local $parameter.v : T0
    val local4
      Local $argument.0 : ULong
    return
      StructInit PinnedPtr : PinnedPtr<T0>
        Local $parameter.raw : ULong
  fun unpin<T : ref>(p: PinnedPtr<T0>): T0 <unsafe>
    val local1
      FieldAccess field 0 : ULong
        Local p : PinnedPtr<T0>
    val local2
      Local $argument.0 : ULong
    return
      Call _unpin<T0> : T0
        Local $parameter.raw : ULong
  fun getGcHandle<T : ref>(v: T0): GcHandle<T0> <unsafe>
    val local1
      Local v : T0
    val local2
      Local $argument.0 : T0
    val local3
      Call _getGcHandle<T0> : ULong
        Local $parameter.v : T0
    val local4
      Local $argument.0 : ULong
    return
      StructInit GcHandle : GcHandle<T0>
        Local $parameter.raw : ULong
  fun releaseGcHandle<T : ref>(h: GcHandle<T0>): T0 <unsafe>
    val local1
      FieldAccess field 0 : ULong
        Local h : GcHandle<T0>
    val local2
      Local $argument.0 : ULong
    return
      Call _releaseGcHandle<T0> : T0
        Local $parameter.raw : ULong
  fun gcCollect(): Unit <intrinsic rt_gc_collect>
  fun gcStats(): ULong <intrinsic rt_gc_stats>
  fun main(): Unit
    val local0
      StringLiteral "hello" : String
    val local1
      Local s : String
    val local2
      Local $argument.0 : String
    val local3
      Call pin<String> : PinnedPtr<String>
        Local $parameter.v : String
    val local4
      Local h : PinnedPtr<String>
    val local5
      Local $argument.0 : PinnedPtr<String>
    val local6
      Call unpin<String> : String
        Local $parameter.p : PinnedPtr<String>
    val local7
      Local s : String
    val local8
      Local $argument.0 : String
    val local9
      Call getGcHandle<String> : GcHandle<String>
        Local $parameter.v : String
    val local10
      Local g : GcHandle<String>
    val local11
      Local $argument.0 : GcHandle<String>
    val local12
      Call releaseGcHandle<String> : String
        Local $parameter.h : GcHandle<String>
    Call gcCollect : Unit
    val local13
      Call gcStats : ULong
  entry main
  instance pin<String>
  instance unpin<String>
  instance getGcHandle<String>
  instance releaseGcHandle<String>
"#;
    assert_eq!(hir::dump_legacy_executable(&module), expected);
}
