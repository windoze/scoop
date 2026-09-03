use super::super::*;

// --- positive: generics ---

#[test]
fn generic_identity_infers_type_arguments() {
    let file = file(vec![
        fun_expr(
            "identity",
            vec!["T"],
            vec![("x", ty_named("T"))],
            Some(ty_named("T")),
            var("x"),
        ),
        fun(
            "main",
            vec![
                stmt(call("println", vec![call("identity", vec![int_lit(42)])])),
                stmt(call("println", vec![call("identity", vec![str_lit("hi")])])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("generic identity must lower");
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
  fun identity<T>(x: T0): T0
    return
      Local x : T0
  fun main(): Unit
    val local0
      IntLiteral 42 : Int
    val local1
      Local $argument.0 : Int
    val local2
      Call identity<Int> : Int
        Local $parameter.x : Int
    val local3
      Local $argument.0 : Int
    Call println<Int> : Unit
      Local $parameter.value : Int
    val local4
      StringLiteral "hi" : String
    val local5
      Local $argument.0 : String
    val local6
      Call identity<String> : String
        Local $parameter.x : String
    val local7
      Local $argument.0 : String
    Call println<String> : Unit
      Local $parameter.value : String
  entry main
  instance identity<Int>
  instance println<Int>
  instance identity<String>
  instance println<String>
"#;
    assert_eq!(hir::dump(&module), expected);
}

#[test]
fn generic_inference_is_independent_of_argument_order() {
    let file = file(vec![
        fun_expr(
            "choose",
            vec!["T"],
            vec![
                ("maybe", ty_nullable(ty_named("T"))),
                ("value", ty_named("T")),
            ],
            Some(ty_named("T")),
            var("value"),
        ),
        fun(
            "main",
            vec![val("x", call("choose", vec![none(), int_lit(7)]))],
        ),
    ]);
    let module = lower_user(file).expect("a later argument must type an earlier None");
    let dump = hir::dump(&module);
    assert!(dump.contains("Call choose<Int> : Int"));
    assert!(dump.contains("VariantConstruct Option.None<Int> : Option<Int>"));
}

#[test]
fn outer_expected_type_fixes_a_return_only_type_parameter() {
    let file = file(vec![
        fun_expr(
            "empty",
            vec!["T"],
            vec![],
            Some(ty_nullable(ty_named("T"))),
            none(),
        ),
        fun(
            "main",
            vec![val_ty(
                "value",
                Some(ty_nullable(ty_named("Int"))),
                call("empty", vec![]),
            )],
        ),
    ]);

    let module = lower_user(file).expect("the call result context must infer T as Int");
    let dump = hir::dump(&module);
    assert!(dump.contains("Call empty<Int> : Option<Int>"), "{dump}");
    assert!(dump.contains("instance empty<Int>"), "{dump}");
}

/// Multiple type parameters bind independently; tuple return types
/// substitute recursively.
#[test]
fn multiple_type_parameters() {
    let file = file(vec![
        fun_expr(
            "pair",
            vec!["T", "U"],
            vec![("a", ty_named("T")), ("b", ty_named("U"))],
            Some(ty_tuple(vec![ty_named("T"), ty_named("U")])),
            tuple_lit(vec![var("a"), var("b")]),
        ),
        fun(
            "main",
            vec![val("p", call("pair", vec![int_lit(1), str_lit("s")]))],
        ),
    ]);
    let module = lower_user(file).expect("two type parameters must lower");
    let dump = hir::dump(&module);
    assert!(
        dump.contains("fun pair<T, U>(a: T0, b: T1): (T0, T1)"),
        "{dump}"
    );
    assert!(
        dump.contains("Call pair<Int, String> : (Int, String)"),
        "{dump}"
    );
    assert!(dump.contains("instance pair<Int, String>"), "{dump}");
}

/// An unconstrained type parameter does not acquire an implicit equality
/// operation. Generic code needs an interface bound that declares the
/// operator member.
#[test]
fn generic_equality_requires_an_operator_bound() {
    let file = file(vec![
        fun_expr(
            "eq",
            vec!["T"],
            vec![("a", ty_named("T")), ("b", ty_named("T"))],
            Some(ty_named("Boolean")),
            binary(BinOp::Eq, var("a"), var("b")),
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("unbounded generic equality must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "type `T` has no member operator `equals` for `==`"
    );
}

/// A generic call inside a generic body records an instantiation
/// request whose type arguments may still mention `Type::Param`;
/// local-concrete HIR resolves them when the requesting instance is
/// materialized, before MIR receives the graph.
#[test]
fn nested_generic_calls_record_param_instantiations() {
    let file = file(vec![
        fun_expr(
            "identity",
            vec!["T"],
            vec![("x", ty_named("T"))],
            Some(ty_named("T")),
            var("x"),
        ),
        fun_sig(
            "twice",
            vec!["U"],
            vec![("v", ty_named("U"))],
            Some(ty_named("U")),
            vec![ret(Some(call(
                "identity",
                vec![call("identity", vec![var("v")])],
            )))],
        ),
        fun(
            "main",
            vec![stmt(call(
                "println",
                vec![call("twice", vec![int_lit(21)])],
            ))],
        ),
    ]);
    let module = lower_user(file).expect("nested generic calls must lower");
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
  fun identity<T>(x: T0): T0
    return
      Local x : T0
  fun twice<U>(v: T0): T0
    val local1
      Local v : T0
    val local2
      Local $argument.0 : T0
    val local3
      Call identity<T0> : T0
        Local $parameter.x : T0
    val local4
      Local $argument.0 : T0
    return
      Call identity<T0> : T0
        Local $parameter.x : T0
  fun main(): Unit
    val local0
      IntLiteral 21 : Int
    val local1
      Local $argument.0 : Int
    val local2
      Call twice<Int> : Int
        Local $parameter.v : Int
    val local3
      Local $argument.0 : Int
    Call println<Int> : Unit
      Local $parameter.value : Int
  entry main
  instance identity<T0>
  instance twice<Int>
  instance println<Int>
"#;
    assert_eq!(hir::dump(&module), expected);
}

/// `Option<T>` in a parameter type binds recursively against an
/// `Option<Int>` argument; the elvis desugaring works over
/// `Option<T0>` inside the generic body.
#[test]
fn generic_option_roundtrip() {
    let file = file(vec![
        fun_sig(
            "unwrapOr",
            vec!["T"],
            vec![
                ("o", ty_nullable(ty_named("T"))),
                ("fallback", ty_named("T")),
            ],
            Some(ty_named("T")),
            vec![ret(Some(elvis(var("o"), var("fallback"))))],
        ),
        fun(
            "main",
            vec![
                val_ty("a", Some(ty_nullable(ty_named("Int"))), some(int_lit(41))),
                stmt(call(
                    "println",
                    vec![call("unwrapOr", vec![var("a"), int_lit(0)])],
                )),
            ],
        ),
    ]);
    let module = lower_user(file).expect("generic Option function must lower");
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
  fun unwrapOr<T>(o: Option<T0>, fallback: T0): T0
    val local2
      Local o : Option<T0>
    if
      IsSome : Boolean
        Local $opt.0 : Option<T0>
      val local3
        Unwrap trap=false : T0
          Local $opt.0 : Option<T0>
    else
      val local3
        Local fallback : T0
    return
      Local $res.1 : T0
  fun main(): Unit
    val local0
      IntLiteral 41 : Int
    val local1
      Local $argument.0 : Int
    val local2
      VariantConstruct Option.Some<Int> : Option<Int>
        Local $parameter._1 : Int
    val local3
      Local a : Option<Int>
    val local4
      IntLiteral 0 : Int
    val local5
      Local $argument.0 : Option<Int>
    val local6
      Local $argument.1 : Int
    val local7
      Call unwrapOr<Int> : Int
        Local $parameter.o : Option<Int>
        Local $parameter.fallback : Int
    val local8
      Local $argument.0 : Int
    Call println<Int> : Unit
      Local $parameter.value : Int
  entry main
  instance unwrapOr<Int>
  instance println<Int>
"#;
    assert_eq!(hir::dump(&module), expected);
}
