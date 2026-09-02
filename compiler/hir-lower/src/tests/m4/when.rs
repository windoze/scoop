use super::*;

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
    let expected = "\
Module
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
      VariantConstruct Color.Red : Color
    when
      Local c : Color
      arm variant0()
        Call println<String> : Unit
          StringLiteral \"red\" : String
      arm variant1()
        Call println<String> : Unit
          StringLiteral \"green\" : String
      arm variant2()
        Call println<String> : Unit
          StringLiteral \"blue\" : String
  entry main
  instance println<String>
";
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
    when
      Local o : Option<Int>
      arm variant0(0: local1) if <guard>
        Call println<Int> : Unit
          Local x : Int
      arm variant0(0: local2)
        Call println<Int> : Unit
          IntLiteral 0 : Int
      arm variant1()
        Call println<String> : Unit
          StringLiteral \"none\" : String
  entry main
  instance println<Int>
  instance println<String>
";
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
      TupleLiteral : (Int, String)
        IntLiteral 1 : Int
        StringLiteral \"a\" : String
    when
      Local t : (Int, String)
      arm (<lit IntLiteral(0)>, _)
        Call println<String> : Unit
          StringLiteral \"zero\" : String
      arm (local1, _)
        Call println<Int> : Unit
          Local n : Int
    val local2
      StructInit Point : Point
        IntLiteral 1 : Int
        IntLiteral 2 : Int
    when
      Local p : Point
      arm struct(0: local3, 1: local4) if <guard>
        Call println<String> : Unit
          StringLiteral \"eq\" : String
      arm struct(0: local5, 1: local6)
        Call println<Int> : Unit
          Local a : Int
  entry main
  instance println<String>
  instance println<Int>
";
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

// --- negative: when ---

#[test]
fn when_subject_must_be_patternable() {
    let file = file(vec![fun(
        "main",
        vec![when_stmt(
            int_lit(1),
            vec![arm(pat_wild(), None, vec![])],
            None,
        )],
    )]);
    let errors = lower_user(file).expect_err("Int subject must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`when` subject must be an enum, tuple or struct, found Int"
    );
}

#[test]
fn unknown_variant_pattern_is_an_error() {
    let file = file(vec![
        color_decl(),
        fun(
            "main",
            vec![
                val("c", field(var("Color"), "Red")),
                when_stmt(
                    var("c"),
                    vec![arm(pat_pos(&["Purple"], vec![], None), None, vec![])],
                    Some(vec![]),
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("unknown variant pattern must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "enum `Color` has no variant `Purple`");
}

#[test]
fn pattern_path_subject_mismatch_is_an_error() {
    let file = file(vec![
        color_decl(),
        fun(
            "main",
            vec![
                val("t", tuple_lit(vec![int_lit(1)])),
                when_stmt(
                    var("t"),
                    vec![arm(pat_pos(&["Color", "Red"], vec![], None), None, vec![])],
                    Some(vec![]),
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("mismatched path must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "pattern `Color.Red` does not match a subject of type (Int)"
    );
}

#[test]
fn variant_pattern_arity_is_an_error() {
    let file = file(vec![
        shape_decl(),
        fun(
            "main",
            vec![
                val("s", call("Shape.Circle", vec![int_lit(1)])),
                when_stmt(
                    var("s"),
                    vec![
                        arm(
                            pat_pos(&["Circle"], vec![pat_bind("r"), pat_bind("q")], None),
                            None,
                            vec![],
                        ),
                        arm(pat_wild(), None, vec![]),
                    ],
                    None,
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("pattern arity must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "pattern has 2 element(s), but variant `Circle` of `Shape` has 1"
    );
}

#[test]
fn tuple_pattern_arity_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val("t", tuple_lit(vec![int_lit(1), int_lit(2)])),
            when_stmt(
                var("t"),
                vec![arm(pat_tuple(vec![pat_bind("x")], None), None, vec![])],
                Some(vec![]),
            ),
        ],
    )]);
    let errors = lower_user(file).expect_err("tuple arity must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "pattern has 1 element(s), but tuple of type (Int, Int) has 2"
    );
}

#[test]
fn tuple_pattern_against_enum_is_an_error() {
    let file = file(vec![
        color_decl(),
        fun(
            "main",
            vec![
                val("c", field(var("Color"), "Red")),
                when_stmt(
                    var("c"),
                    vec![arm(pat_tuple(vec![pat_bind("x")], None), None, vec![])],
                    Some(vec![]),
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("tuple pattern on enum must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "tuple pattern does not match a subject of type Color"
    );
}

#[test]
fn named_pattern_missing_rest_is_an_error() {
    let file = file(vec![
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
        ),
        fun(
            "main",
            vec![
                val("p", call("Point", vec![int_lit(1), int_lit(2)])),
                when_stmt(
                    var("p"),
                    vec![arm(
                        pat_named(&["Point"], vec![("x", None)], None),
                        None,
                        vec![],
                    )],
                    Some(vec![]),
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("missing `..` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "pattern does not list all fields of struct `Point`; add `..` to ignore the rest"
    );
}

#[test]
fn named_pattern_unknown_field_is_an_error() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("p", call("Point", vec![int_lit(1)])),
                when_stmt(
                    var("p"),
                    vec![arm(
                        pat_named(&["Point"], vec![("x", None), ("z", None)], None),
                        None,
                        vec![],
                    )],
                    Some(vec![]),
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("unknown field must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "struct `Point` has no field `z`");
}

#[test]
fn named_pattern_underscore_is_an_error() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("p", call("Point", vec![int_lit(1)])),
                when_stmt(
                    var("p"),
                    vec![arm(
                        pat_named(&["Point"], vec![("_", None)], None),
                        None,
                        vec![],
                    )],
                    Some(vec![]),
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("`_` in field pattern must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`_` is not allowed in a field pattern");
}

#[test]
fn duplicate_field_in_pattern_is_an_error() {
    let file = file(vec![
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("p", call("Point", vec![int_lit(1)])),
                when_stmt(
                    var("p"),
                    vec![arm(
                        pat_named(&["Point"], vec![("x", None), ("x", Some("y"))], None),
                        None,
                        vec![],
                    )],
                    Some(vec![]),
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("duplicate field must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate field `x` in pattern");
}

#[test]
fn positional_pattern_on_named_variant_is_an_error() {
    let file = file(vec![
        shape_decl(),
        fun(
            "main",
            vec![
                val("s", call("Shape.Named", vec![int_lit(1), int_lit(2)])),
                when_stmt(
                    var("s"),
                    vec![
                        arm(
                            pat_pos(&["Named"], vec![pat_bind("w"), pat_bind("h")], None),
                            None,
                            vec![],
                        ),
                        arm(pat_wild(), None, vec![]),
                    ],
                    None,
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("positional on named must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "variant `Named` of `Shape` has named fields; use a named field pattern"
    );
}

#[test]
fn named_pattern_on_positional_variant_is_an_error() {
    let file = file(vec![
        shape_decl(),
        fun(
            "main",
            vec![
                val("s", call("Shape.Circle", vec![int_lit(1)])),
                when_stmt(
                    var("s"),
                    vec![
                        arm(pat_named(&["Circle"], vec![], Some(sp())), None, vec![]),
                        arm(pat_wild(), None, vec![]),
                    ],
                    None,
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("named on positional must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "variant `Circle` of `Shape` has no named fields; use a positional pattern"
    );
}

#[test]
fn field_pattern_without_name_on_enum_is_an_error() {
    let file = file(vec![
        shape_decl(),
        fun(
            "main",
            vec![
                val("s", call("Shape.Circle", vec![int_lit(1)])),
                when_stmt(
                    var("s"),
                    vec![arm(pat_named(&[], vec![], Some(sp())), None, vec![])],
                    Some(vec![]),
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("nameless field pattern must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "a field pattern without a type name can only match a struct, found Shape"
    );
}

#[test]
fn when_guard_must_be_boolean() {
    let file = file(vec![
        color_decl(),
        fun(
            "main",
            vec![
                val("c", field(var("Color"), "Red")),
                when_stmt(
                    var("c"),
                    vec![arm(pat_bind("Red"), Some(int_lit(1)), vec![])],
                    Some(vec![]),
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("Int guard must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "when guard must be Boolean, found Int");
}

#[test]
fn desugaring_in_when_guard_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty(
                "o",
                Some(ty_nullable(ty_named("Boolean"))),
                some(bool_lit(true)),
            ),
            when_stmt(
                var("o"),
                vec![arm(
                    pat_pos(&["Some"], vec![pat_bind("b")], None),
                    Some(elvis(var("o"), bool_lit(false))),
                    vec![],
                )],
                Some(vec![]),
            ),
        ],
    )]);
    let errors = lower_user(file).expect_err("elvis in guard must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`?.` and `?:` are not allowed in a when guard"
    );
}

#[test]
fn non_exhaustive_when_reports_missing_variants() {
    let file = file(vec![
        color_decl(),
        fun(
            "main",
            vec![
                val("c", field(var("Color"), "Red")),
                when_stmt(var("c"), vec![arm(pat_bind("Red"), None, vec![])], None),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("non-exhaustive when must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "non-exhaustive when: missing variant(s) `Green`, `Blue`"
    );
}

#[test]
fn guarded_arm_does_not_count_for_exhaustiveness() {
    let file = file(vec![
        color_decl(),
        fun(
            "main",
            vec![
                val("c", field(var("Color"), "Red")),
                when_stmt(
                    var("c"),
                    vec![
                        arm(pat_bind("Red"), Some(bool_lit(true)), vec![]),
                        arm(pat_bind("Green"), None, vec![]),
                        arm(pat_bind("Blue"), None, vec![]),
                    ],
                    None,
                ),
            ],
        ),
    ]);
    let errors = lower_user(file).expect_err("guarded arm must not count");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "non-exhaustive when: missing variant(s) `Red`"
    );
}

#[test]
fn non_exhaustive_tuple_when_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val("t", tuple_lit(vec![int_lit(1), int_lit(2)])),
            when_stmt(
                var("t"),
                // A literal makes the arm refutable.
                vec![arm(
                    pat_tuple(vec![pat_lit(int_lit(0)), pat_bind("x")], None),
                    None,
                    vec![],
                )],
                None,
            ),
        ],
    )]);
    let errors = lower_user(file).expect_err("non-exhaustive tuple when must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "non-exhaustive when: add a catch-all pattern or an `else` branch"
    );
}
