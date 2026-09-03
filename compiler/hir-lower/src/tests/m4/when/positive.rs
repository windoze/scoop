use super::super::*;

// --- positive: when ---

#[test]
fn when_over_enum_with_bare_and_qualified_variants() {
    let file = file(vec![
        color_decl(),
        fun(
            "main",
            vec![
                val("c", field(var("Color"), "Red")),
                when_stmt(
                    var("c"),
                    vec![
                        // Bare unit variant (spec 5.1: prefix optional).
                        arm(
                            pat_bind("Red"),
                            None,
                            vec![stmt(call("println", vec![str_lit("red")]))],
                        ),
                        // Qualified unit variant.
                        arm(
                            pat_pos(&["Color", "Green"], vec![], None),
                            None,
                            vec![stmt(call("println", vec![str_lit("green")]))],
                        ),
                        arm(
                            pat_bind("Blue"),
                            None,
                            vec![stmt(call("println", vec![str_lit("blue")]))],
                        ),
                    ],
                    None,
                ),
            ],
        ),
    ]);
    let module = lower_user(file).expect("when over Color must lower");
    let expected = r#"Module
  enum Option<T>
    Some(_1: T0)
    None()
  enum Color
    Red()
    Green()
    Blue()
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
      VariantConstruct Color.Red : Color
    when
      Local c : Color
      arm variant0()
        val local1
          StringLiteral "red" : String
        val local2
          Local $argument.0 : String
        Call println<String> : Unit
          Local $parameter.value : String
      arm variant1()
        val local3
          StringLiteral "green" : String
        val local4
          Local $argument.0 : String
        Call println<String> : Unit
          Local $parameter.value : String
      arm variant2()
        val local5
          StringLiteral "blue" : String
        val local6
          Local $argument.0 : String
        Call println<String> : Unit
          Local $parameter.value : String
  entry main
  instance println<String>
"#;
    assert_eq!(hir::dump(&module), expected);
}

/// `when` over the core `Option`: positional variant pattern with a
/// binding, a guard using the binding, and the bare `None` unit
/// variant. Unguarded arms cover both variants, so no `else` is needed
/// (the guarded arm does not count).
#[test]
fn when_over_option_with_guard() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("o", Some(ty_nullable(ty_named("Int"))), some(int_lit(41))),
            when_stmt(
                var("o"),
                vec![
                    arm(
                        pat_pos(&["Some"], vec![pat_bind("x")], None),
                        Some(binary(BinOp::Gt, var("x"), int_lit(0))),
                        vec![stmt(call("println", vec![var("x")]))],
                    ),
                    arm(
                        pat_pos(&["Some"], vec![pat_bind("x")], None),
                        None,
                        vec![stmt(call("println", vec![int_lit(0)]))],
                    ),
                    arm(
                        pat_bind("None"),
                        None,
                        vec![stmt(call("println", vec![str_lit("none")]))],
                    ),
                ],
                None,
            ),
        ],
    )]);
    let module = lower_user(file).expect("when over Option must lower");
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
      IntLiteral 41 : Int
    val local1
      Local $argument.0 : Int
    val local2
      VariantConstruct Option.Some<Int> : Option<Int>
        Local $parameter._1 : Int
    when
      Local o : Option<Int>
      arm variant0(0: local3) if <guard>
        guard setup
          val local4
            Local x : Int
          val local5
            IntLiteral 0 : Int
          val local6
            Local $argument.0 : Int
        guard condition
          Binary Gt : Boolean
            PrimitiveBinary IntCompareTo : Int
              Local $receiver : Int
              Local $parameter.other : Int
            IntLiteral 0 : Int
        val local7
          Local x : Int
        val local8
          Local $argument.0 : Int
        Call println<Int> : Unit
          Local $parameter.value : Int
      arm variant0(0: local9)
        val local10
          IntLiteral 0 : Int
        val local11
          Local $argument.0 : Int
        Call println<Int> : Unit
          Local $parameter.value : Int
      arm variant1()
        val local12
          StringLiteral "none" : String
        val local13
          Local $argument.0 : String
        Call println<String> : Unit
          Local $parameter.value : String
  entry main
  instance println<Int>
  instance println<String>
"#;
    assert_eq!(hir::dump(&module), expected);
}

/// Tuple and struct subjects: positional and field patterns, literals,
/// `..`, an irrefutable catch-all (no `else` needed).
#[test]
fn when_over_tuple_and_struct() {
    let file = file(vec![
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
        ),
        fun(
            "main",
            vec![
                val("t", tuple_lit(vec![int_lit(1), str_lit("a")])),
                when_stmt(
                    var("t"),
                    vec![
                        // Literal first element, wildcard second.
                        arm(
                            pat_tuple(vec![pat_lit(int_lit(0)), pat_wild()], None),
                            None,
                            vec![stmt(call("println", vec![str_lit("zero")]))],
                        ),
                        // Irrefutable catch-all with `..`.
                        arm(
                            pat_tuple(vec![pat_bind("n")], Some(trailing_rest())),
                            None,
                            vec![stmt(call("println", vec![var("n")]))],
                        ),
                    ],
                    None,
                ),
                val("p", call("Point", vec![int_lit(1), int_lit(2)])),
                when_stmt(
                    var("p"),
                    vec![
                        // Field pattern with rename (all fields listed,
                        // so no `..`).
                        arm(
                            pat_named(&["Point"], vec![("x", None), ("y", Some("yy"))], None),
                            Some(binary(BinOp::Eq, var("x"), var("yy"))),
                            vec![stmt(call("println", vec![str_lit("eq")]))],
                        ),
                        // Positional struct pattern without prefix.
                        arm(
                            pat_tuple(vec![pat_bind("a"), pat_bind("b")], None),
                            None,
                            vec![stmt(call("println", vec![var("a")]))],
                        ),
                    ],
                    None,
                ),
            ],
        ),
    ]);
    let module = lower_user(file).expect("tuple/struct when must lower");
    let expected = r#"Module
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
        IntLiteral 1 : Int
        StringLiteral "a" : String
    when
      Local t : (Int, String)
      arm (<lit IntLiteral(0)>, _)
        val local1
          StringLiteral "zero" : String
        val local2
          Local $argument.0 : String
        Call println<String> : Unit
          Local $parameter.value : String
      arm (local3, _)
        val local4
          Local n : Int
        val local5
          Local $argument.0 : Int
        Call println<Int> : Unit
          Local $parameter.value : Int
    val local6
      IntLiteral 1 : Int
    val local7
      IntLiteral 2 : Int
    val local8
      Local $argument.0 : Int
    val local9
      Local $argument.1 : Int
    val local10
      StructInit Point : Point
        Local $parameter.x : Int
        Local $parameter.y : Int
    when
      Local p : Point
      arm struct(0: local11, 1: local12) if <guard>
        guard condition
          MethodCall Int.equals : Boolean
            Local x : Int
            Local yy : Int
        val local13
          StringLiteral "eq" : String
        val local14
          Local $argument.0 : String
        Call println<String> : Unit
          Local $parameter.value : String
      arm struct(0: local15, 1: local16)
        val local17
          Local a : Int
        val local18
          Local $argument.0 : Int
        Call println<Int> : Unit
          Local $parameter.value : Int
  entry main
  instance println<String>
  instance println<Int>
"#;
    assert_eq!(hir::dump(&module), expected);
}

/// An unguarded binding arm is a catch-all (spec 5 "binding
/// priority"): it makes any `when` exhaustive.
#[test]
fn when_catch_all_binding_arm_is_exhaustive() {
    let file = file(vec![
        color_decl(),
        fun(
            "main",
            vec![
                val("c", field(var("Color"), "Red")),
                when_stmt(
                    var("c"),
                    vec![
                        arm(
                            pat_bind("Red"),
                            None,
                            vec![stmt(call("println", vec![str_lit("red")]))],
                        ),
                        arm(
                            pat_bind("other"),
                            None,
                            vec![stmt(call("println", vec![str_lit("other")]))],
                        ),
                    ],
                    None,
                ),
            ],
        ),
    ]);
    lower_user(file).expect("catch-all binding arm must lower");
}

/// `else` rescues an otherwise non-exhaustive `when`.
#[test]
fn when_else_covers_missing_variants() {
    let file = file(vec![
        color_decl(),
        fun(
            "main",
            vec![
                val("c", field(var("Color"), "Red")),
                when_stmt(
                    var("c"),
                    vec![arm(
                        pat_bind("Red"),
                        None,
                        vec![stmt(call("println", vec![str_lit("red")]))],
                    )],
                    Some(vec![stmt(call("println", vec![str_lit("other")]))]),
                ),
            ],
        ),
    ]);
    let module = lower_user(file).expect("else branch must lower");
    let dump = hir::dump(&module);
    assert!(dump.contains("  else\n"), "{dump}");
}
