use super::super::*;

// --- positive: Option and nullables ---

#[test]
fn some_none_and_nullable_annotations() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("a", Some(ty_nullable(ty_named("Int"))), some(int_lit(41))),
            val_ty("b", Some(ty_nullable(ty_named("Int"))), none()),
            // `T??` is `Option<Option<T>>` and does not collapse; the
            // expected type lets `Some(None)` type the inner `None`.
            val_ty(
                "c",
                Some(ty_nullable(ty_nullable(ty_named("Int")))),
                some(none()),
            ),
        ],
    )]);
    let module = lower_user(file).expect("Option constructors must lower");
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
      VariantConstruct Option.Some<Int> : Option<Int>
        IntLiteral 41 : Int
    val local1
      VariantConstruct Option.None<Int> : Option<Int>
    val local2
      VariantConstruct Option.Some<Option<Int>> : Option<Option<Int>>
        VariantConstruct Option.None<Int> : Option<Int>
  entry main
";
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn safe_field_access_desugars_to_hidden_locals() {
    let file = file(vec![
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
        ),
        fun(
            "main",
            vec![
                val_ty(
                    "p",
                    Some(ty_nullable(ty_named("Point"))),
                    some(call("Point", vec![int_lit(1), int_lit(2)])),
                ),
                val("x", safe_field(var("p"), "x")),
            ],
        ),
    ]);
    let module = lower_user(file).expect("safe field access must lower");
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
      VariantConstruct Option.Some<Point> : Option<Point>
        StructInit Point : Point
          IntLiteral 1 : Int
          IntLiteral 2 : Int
    val local1
      Local p : Option<Point>
    if
      IsSome : Boolean
        Local $opt.0 : Option<Point>
      val local2
        SomeWrap : Option<Int>
          FieldAccess field 0 : Int
            Unwrap trap=false : Point
              Local $opt.0 : Option<Point>
    else
      val local2
        NoneLiteral : Option<Int>
    val local3
      Local $res.1 : Option<Int>
  entry main
";
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn elvis_desugars_to_hidden_locals() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("a", Some(ty_nullable(ty_named("Int"))), some(int_lit(41))),
            val("b", elvis(var("a"), int_lit(0))),
        ],
    )]);
    let module = lower_user(file).expect("elvis must lower");
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
      VariantConstruct Option.Some<Int> : Option<Int>
        IntLiteral 41 : Int
    val local1
      Local a : Option<Int>
    if
      IsSome : Boolean
        Local $opt.0 : Option<Int>
      val local2
        Unwrap trap=false : Int
          Local $opt.0 : Option<Int>
    else
      val local2
        IntLiteral 0 : Int
    val local3
      Local $res.1 : Int
  entry main
";
    assert_eq!(hir::dump(&module), expected);
}

/// `w?.p?.x` chains: `w?.p` is an `Option<Point>`, whose `?.x` is an
/// `Option<Int>` — the desugarings nest through the sink.
#[test]
fn chained_safe_field_access() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        struct_decl("Wrap", vec![("p", ty_named("Point"))]),
        fun(
            "main",
            vec![
                val_ty(
                    "w",
                    Some(ty_nullable(ty_named("Wrap"))),
                    some(call("Wrap", vec![call("Point", vec![int_lit(1)])])),
                ),
                val("x", safe_field(safe_field(var("w"), "p"), "x")),
            ],
        ),
    ]);
    let module = lower_user(file).expect("chained safe access must lower");
    let dump = hir::dump(&module);
    // w?.p : Option<Point>, then ?.x : Option<Int>. Hidden locals are
    // numbered per body: w, $opt.0, $res.1, $opt.2, $res.3, x.
    assert!(
        dump.contains("val local1\n      Local w : Option<Wrap>"),
        "{dump}"
    );
    assert!(
        dump.contains("val local2\n        SomeWrap : Option<Point>"),
        "{dump}"
    );
    assert!(
        dump.contains("val local3\n      Local $res.1 : Option<Point>"),
        "{dump}"
    );
    assert!(
        dump.contains("val local5\n      Local $res.3 : Option<Int>"),
        "{dump}"
    );
}

#[test]
fn null_assert_unwraps_with_trap() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("a", Some(ty_nullable(ty_named("Int"))), some(int_lit(41))),
            val("x", null_assert(var("a"))),
            stmt(call("println", vec![var("x")])),
        ],
    )]);
    let module = lower_user(file).expect("null assert must lower");
    let dump = hir::dump(&module);
    assert!(
        dump.contains("val local1\n      Unwrap trap=true : Int\n        Local a : Option<Int>"),
        "{dump}"
    );
}

/// `x == None` and `None == x`: the literal takes the other side's
/// type from the expected-type hint.
#[test]
fn none_equality_comparison() {
    for comparison in [
        binary(BinOp::Eq, var("a"), none()),
        binary(BinOp::Ne, none(), var("a")),
    ] {
        let file = file(vec![fun(
            "main",
            vec![
                val_ty("a", Some(ty_nullable(ty_named("Int"))), none()),
                if_stmt(
                    comparison,
                    vec![stmt(call("println", vec![str_lit("empty")]))],
                    None,
                ),
            ],
        )]);
        let module = lower_user(file).expect("`== None` must lower");
        let dump = hir::dump(&module);
        assert!(
            dump.contains("VariantConstruct Option.None<Int> : Option<Int>"),
            "{dump}"
        );
    }
}
