//! M4 tests: enum declarations (four variant forms), variant
//! construction, `when` with guards and exhaustiveness, destructuring
//! declarations, intrinsics and the sysroot (multi-file) frame. Golden
//! dumps lock the enum/when/destructuring output structure; one
//! negative test per diagnostic.

use super::*;

fn color_decl() -> Decl {
    enum_decl(
        "Color",
        vec![],
        vec![
            variant_unit("Red"),
            variant_unit("Green"),
            variant_unit("Blue"),
        ],
    )
}

/// `enum Shape { Circle(Int), Named { w: Int, h: Int }, WithDefault(val d: Int = 0) }`.
fn shape_decl() -> Decl {
    enum_decl(
        "Shape",
        vec![],
        vec![
            variant_positional("Circle", vec![ty_named("Int")]),
            variant_named(
                "Named",
                vec![("w", ty_named("Int")), ("h", ty_named("Int"))],
            ),
            variant_constructor(
                "WithDefault",
                vec![("d", ty_named("Int"), Some(int_lit(0)))],
            ),
        ],
    )
}

// --- positive: enum declarations ---

#[test]
fn enum_declaration_all_variant_forms() {
    let file = file(vec![
        color_decl(),
        shape_decl(),
        fun(
            "main",
            vec![
                val("c", field(var("Color"), "Red")),
                val("s", call("Shape.Circle", vec![int_lit(1)])),
                // Constructor-style default fills the trailing field.
                val("w", call("Shape.WithDefault", vec![])),
                // Named variants construct positionally in M4 (the AST
                // has no named-argument form).
                val("n", call("Shape.Named", vec![int_lit(2), int_lit(3)])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("enum program must lower");
    let expected = "\
Module
  enum Option<T>
    Some(_1: T0)
    None()
  enum Color
    Red()
    Green()
    Blue()
  enum Shape
    Circle(_1: Int)
    Named(w: Int, h: Int)
    WithDefault(d: Int)
  open class Throwable()
  class IllegalStateException()
  interface Continuation<in T>
    fun resume(value: T0): Unit
    fun resumeWithException(exception: Throwable): Unit
  interface SuspendTask<out T>
    suspend fun run(): T0
  interface SuspendRegistration<out T>
    fun register(continuation: Continuation<T0>): Unit
  fun startCoroutine<T>(): Unit <intrinsic coroutine_start>
  suspend fun suspendCoroutine<T>(): T0 <intrinsic coroutine_suspend>
  fun write(): Unit <intrinsic rt_write>
  fun print(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    return
  fun println(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    Call write : Unit
      StringLiteral \"\\n\" : String
  fun main(): Unit
    val local0
      VariantConstruct Color.Red : Color
    val local1
      VariantConstruct Shape.Circle : Shape
        IntLiteral 1 : Int
    val local2
      VariantConstruct Shape.WithDefault : Shape
        IntLiteral 0 : Int
    val local3
      VariantConstruct Shape.Named : Shape
        IntLiteral 2 : Int
        IntLiteral 3 : Int
  entry main
";
    assert_eq!(hir::dump(&module), expected);
}

/// A generic enum with several instantiations coexisting; `T?`
/// interning keeps `Option<Int>` a single `TypeId`.
#[test]
fn generic_enum_instantiations_and_interning() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("a", Some(ty_nullable(ty_named("Int"))), some(int_lit(1))),
            val_ty("b", Some(ty_nullable(ty_named("Int"))), none()),
            val_ty(
                "c",
                Some(ty_nullable(ty_named("String"))),
                some(str_lit("x")),
            ),
            // `==` on enum values of the same type is allowed (the
            // expansion happens in MIR).
            val("same", binary(BinOp::Eq, var("a"), var("b"))),
        ],
    )]);
    let module = lower_user(file).expect("generic enum program must lower");
    let body = match &module.functions[module.entry].kind {
        FunctionKind::User(body) => body,
        FunctionKind::Intrinsic(_) => panic!("main is a user function"),
    };
    // `val a` and `val b` share the interned `Option<Int>` type.
    let locals: Vec<TypeId> = body.locals.iter().map(|(_, local)| local.ty).collect();
    assert_eq!(locals[0], locals[1], "Option<Int> must be interned");
    assert_ne!(locals[0], locals[2], "Option<String> is a different type");
    let dump = hir::dump(&module);
    assert!(
        dump.contains("VariantConstruct Option.None<Int> : Option<Int>"),
        "{dump}"
    );
    assert!(dump.contains("Binary Eq : Boolean"), "{dump}");
}

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
  class IllegalStateException()
  interface Continuation<in T>
    fun resume(value: T0): Unit
    fun resumeWithException(exception: Throwable): Unit
  interface SuspendTask<out T>
    suspend fun run(): T0
  interface SuspendRegistration<out T>
    fun register(continuation: Continuation<T0>): Unit
  fun startCoroutine<T>(): Unit <intrinsic coroutine_start>
  suspend fun suspendCoroutine<T>(): T0 <intrinsic coroutine_suspend>
  fun write(): Unit <intrinsic rt_write>
  fun print(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    return
  fun println(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    Call write : Unit
      StringLiteral \"\\n\" : String
  fun main(): Unit
    val local0
      VariantConstruct Color.Red : Color
    when
      Local c : Color
      arm variant0()
        Call println : Unit
          StringLiteral \"red\" : Any
      arm variant1()
        Call println : Unit
          StringLiteral \"green\" : Any
      arm variant2()
        Call println : Unit
          StringLiteral \"blue\" : Any
  entry main
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
  class IllegalStateException()
  interface Continuation<in T>
    fun resume(value: T0): Unit
    fun resumeWithException(exception: Throwable): Unit
  interface SuspendTask<out T>
    suspend fun run(): T0
  interface SuspendRegistration<out T>
    fun register(continuation: Continuation<T0>): Unit
  fun startCoroutine<T>(): Unit <intrinsic coroutine_start>
  suspend fun suspendCoroutine<T>(): T0 <intrinsic coroutine_suspend>
  fun write(): Unit <intrinsic rt_write>
  fun print(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    return
  fun println(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    Call write : Unit
      StringLiteral \"\\n\" : String
  fun main(): Unit
    val local0
      VariantConstruct Option.Some<Int> : Option<Int>
        IntLiteral 41 : Int
    when
      Local o : Option<Int>
      arm variant0(0: local1) if <guard>
        Call println : Unit
          Box : Any
            Local x : Int
      arm variant0(0: local2)
        Call println : Unit
          Box : Any
            IntLiteral 0 : Int
      arm variant1()
        Call println : Unit
          StringLiteral \"none\" : Any
  entry main
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
  class IllegalStateException()
  interface Continuation<in T>
    fun resume(value: T0): Unit
    fun resumeWithException(exception: Throwable): Unit
  interface SuspendTask<out T>
    suspend fun run(): T0
  interface SuspendRegistration<out T>
    fun register(continuation: Continuation<T0>): Unit
  fun startCoroutine<T>(): Unit <intrinsic coroutine_start>
  suspend fun suspendCoroutine<T>(): T0 <intrinsic coroutine_suspend>
  fun write(): Unit <intrinsic rt_write>
  fun print(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    return
  fun println(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
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
        Call println : Unit
          StringLiteral \"zero\" : Any
      arm (local1, _)
        Call println : Unit
          Box : Any
            Local n : Int
    val local2
      StructInit Point : Point
        IntLiteral 1 : Int
        IntLiteral 2 : Int
    when
      Local p : Point
      arm struct(0: local3, 1: local4) if <guard>
        Call println : Unit
          StringLiteral \"eq\" : Any
      arm struct(0: local5, 1: local6)
        Call println : Unit
          Box : Any
            Local a : Int
  entry main
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

// --- positive: destructuring declarations ---

#[test]
fn destructuring_declarations() {
    let file = file(vec![
        struct_decl(
            "Point",
            vec![("x", ty_named("Int")), ("y", ty_named("Int"))],
        ),
        fun(
            "main",
            vec![
                // Tuple, with `..` skipping the middle elements.
                val(
                    "t",
                    tuple_lit(vec![int_lit(1), int_lit(2), int_lit(3), int_lit(4)]),
                ),
                val_pat(
                    false,
                    ast::Pattern::Tuple {
                        elements: vec![
                            pat_bind_at("a", Span::new(10, 11)),
                            pat_bind_at("b", Span::new(30, 31)),
                        ],
                        rest: Some(Span::new(20, 22)),
                        span: sp(),
                    },
                    None,
                    var("t"),
                ),
                // Struct field pattern with rename and `..`.
                val("p", call("Point", vec![int_lit(1), int_lit(2)])),
                val_pat(
                    false,
                    pat_named(&["Point"], vec![("x", Some("px"))], Some(sp())),
                    None,
                    var("p"),
                ),
                // Struct positional pattern, `var` bindings are mutable.
                var_pat2(pat_tuple(vec![pat_bind("qx"), pat_wild()], None), var("p")),
                // Nested tuple-in-tuple.
                val(
                    "nested",
                    tuple_lit(vec![tuple_lit(vec![int_lit(1), int_lit(2)]), str_lit("s")]),
                ),
                val_pat(
                    false,
                    pat_tuple(
                        vec![
                            pat_tuple(vec![pat_bind("m"), pat_bind("n")], None),
                            pat_wild(),
                        ],
                        None,
                    ),
                    None,
                    var("nested"),
                ),
                stmt(call("println", vec![var("a")])),
                stmt(call("println", vec![var("b")])),
                stmt(call("println", vec![var("px")])),
                stmt(call("println", vec![var("m")])),
            ],
        ),
    ]);
    let module = lower_user(file).expect("destructuring must lower");
    let expected = "\
Module
  struct Point
    field x: Int
    field y: Int
  enum Option<T>
    Some(_1: T0)
    None()
  open class Throwable()
  class IllegalStateException()
  interface Continuation<in T>
    fun resume(value: T0): Unit
    fun resumeWithException(exception: Throwable): Unit
  interface SuspendTask<out T>
    suspend fun run(): T0
  interface SuspendRegistration<out T>
    fun register(continuation: Continuation<T0>): Unit
  fun startCoroutine<T>(): Unit <intrinsic coroutine_start>
  suspend fun suspendCoroutine<T>(): T0 <intrinsic coroutine_suspend>
  fun write(): Unit <intrinsic rt_write>
  fun print(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    return
  fun println(message: Any): Unit
    Call write : Unit
      MethodCall Any.toString : String
        Local message : Any
    Call write : Unit
      StringLiteral \"\\n\" : String
  fun main(): Unit
    val local0
      TupleLiteral : (Int, Int, Int, Int)
        IntLiteral 1 : Int
        IntLiteral 2 : Int
        IntLiteral 3 : Int
        IntLiteral 4 : Int
    val (local1, _, _, local2)
      Local t : (Int, Int, Int, Int)
    val local3
      StructInit Point : Point
        IntLiteral 1 : Int
        IntLiteral 2 : Int
    val struct(0: local4)
      Local p : Point
    val struct(0: local5, 1: _)
      Local p : Point
    val local6
      TupleLiteral : ((Int, Int), String)
        TupleLiteral : (Int, Int)
          IntLiteral 1 : Int
          IntLiteral 2 : Int
        StringLiteral \"s\" : String
    val ((local7, local8), _)
      Local nested : ((Int, Int), String)
    Call println : Unit
      Box : Any
        Local a : Int
    Call println : Unit
      Box : Any
        Local b : Int
    Call println : Unit
      Box : Any
        Local px : Int
    Call println : Unit
      Box : Any
        Local m : Int
  entry main
";
    assert_eq!(hir::dump(&module), expected);
}

/// `var` destructuring: bindings are mutable.
fn var_pat2(target: ast::Pattern, init: Expr) -> Statement {
    val_pat(true, target, None, init)
}

// --- positive: intrinsics and the sysroot frame ---

#[test]
fn intrinsic_functions_are_marked() {
    let module = lower_user(file(vec![fun("main", vec![])])).expect("must lower");
    let write = module
        .top_level
        .iter()
        .map(|id| &module.functions[*id])
        .find(|f| f.name == "write")
        .expect("core declares write");
    assert!(matches!(
        write.kind,
        FunctionKind::Intrinsic(ref name) if name == "rt_write"
    ));
}

/// Multiple core files share the declaration scope (sysroot frame):
/// an enum declared in one core file is visible in the user file.
#[test]
fn multiple_core_files_share_scope() {
    let core_extra = file(vec![color_decl()]);
    let user = file(vec![fun(
        "main",
        vec![val("c", field(var("Color"), "Green"))],
    )]);
    let module =
        lower(&[core_file(), core_extra, user]).expect("multi-core-file program must lower");
    let dump = hir::dump(&module);
    assert!(
        dump.contains("VariantConstruct Color.Green : Color"),
        "{dump}"
    );
}

/// A diagnostic in a core file carries that file's index.
#[test]
fn core_file_diagnostic_carries_file_index() {
    let bad_core = file(vec![
        enum_decl(
            "Option",
            vec!["T"],
            vec![
                variant_positional("Some", vec![ty_named("T")]),
                variant_unit("None"),
            ],
        ),
        struct_decl("Point", vec![]),
        struct_decl("Point", vec![]),
    ]);
    let errors = lower(&[bad_core, file(vec![fun("main", vec![])])])
        .expect_err("duplicate struct in core must fail");
    // The same run also reports the missing core `Throwable` (M8);
    // this test is about the core file index of the duplicate.
    let duplicate = errors
        .iter()
        .find(|e| e.message == "duplicate struct `Point`")
        .expect("the duplicate struct must be diagnosed");
    assert_eq!(duplicate.file, 0);
}

// --- negative: enum declarations ---

#[test]
fn duplicate_enum_is_an_error() {
    let file = file(vec![color_decl(), color_decl(), fun("main", vec![])]);
    let errors = lower_user(file).expect_err("duplicate enum must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate enum `Color`");
}

#[test]
fn enum_struct_name_collision_is_an_error() {
    for decls in [
        vec![
            struct_decl("Point", vec![]),
            enum_decl("Point", vec![], vec![]),
        ],
        vec![
            enum_decl("Point", vec![], vec![]),
            struct_decl("Point", vec![]),
        ],
    ] {
        let mut decls = decls;
        decls.push(fun("main", vec![]));
        let errors = lower_user(file(decls)).expect_err("name collision must fail");
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].message.starts_with("duplicate type `Point`"),
            "{}",
            errors[0].message
        );
    }
}

#[test]
fn duplicate_variant_is_an_error() {
    let file = file(vec![
        enum_decl(
            "Color",
            vec![],
            vec![variant_unit("Red"), variant_unit("Red")],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("duplicate variant must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate variant `Red` in enum `Color`");
}

#[test]
fn duplicate_variant_field_is_an_error() {
    let file = file(vec![
        enum_decl(
            "Shape",
            vec![],
            vec![variant_named(
                "Named",
                vec![("w", ty_named("Int")), ("w", ty_named("Int"))],
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("duplicate field must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate field `w` in variant `Named`");
}

#[test]
fn duplicate_enum_type_parameter_is_an_error() {
    let file = file(vec![
        enum_decl("Pair", vec!["T", "T"], vec![]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("duplicate type parameter must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "duplicate type parameter `T`");
}

#[test]
fn unknown_variant_field_type_is_an_error() {
    let file = file(vec![
        enum_decl(
            "Shape",
            vec![],
            vec![variant_positional("Circle", vec![ty_named("Foo")])],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("unknown field type must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown type `Foo`");
}

#[test]
fn non_literal_variant_default_is_an_error() {
    let file = file(vec![
        enum_decl(
            "Shape",
            vec![],
            vec![variant_constructor(
                "WithDefault",
                vec![("d", ty_named("Int"), Some(call("f", vec![])))],
            )],
        ),
        fun("f", vec![]),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("non-literal default must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "default value of field `d` in variant `WithDefault` must be a literal"
    );
}

#[test]
fn variant_default_type_mismatch_is_an_error() {
    let file = file(vec![
        enum_decl(
            "Shape",
            vec![],
            vec![variant_constructor(
                "WithDefault",
                vec![("d", ty_named("Int"), Some(str_lit("x")))],
            )],
        ),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("default mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "default value of field `d` in variant `WithDefault` must be of type Int, found String"
    );
}

#[test]
fn named_variant_default_is_an_error() {
    // The parser only produces defaults on constructor-style variants;
    // HIR rejects the shape too.
    let file = file(vec![
        Decl::Enum(ast::EnumDecl {
            name: ident("Shape"),
            type_params: vec![],
            methods: vec![],
            interfaces: vec![],
            variants: vec![VariantDecl {
                name: ident("Named"),
                kind: VariantDeclKind::Named(vec![VariantFieldDecl {
                    name: ident("w"),
                    ty: ty_named("Int"),
                    default: Some(int_lit(0)),
                    span: sp(),
                }]),
                span: sp(),
            }],
            span: sp(),
        }),
        fun("main", vec![]),
    ]);
    let errors = lower_user(file).expect_err("named-variant default must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "default value of field `w` in variant `Named` is only allowed on constructor-style variants"
    );
}

// --- negative: the core Option contract ---

#[test]
fn missing_core_option_is_an_error() {
    // A user file alone (no core) has no `Option<T>`.
    let errors =
        lower(&[file(vec![fun("main", vec![])])]).expect_err("missing core Option must fail");
    // The same run also reports the missing core `Throwable` (M8).
    assert!(
        errors
            .iter()
            .any(|e| e.message == "scoop.core must define an enum `Option<T>`" && e.file == 0),
        "{errors:?}"
    );
}

#[test]
fn core_option_must_have_one_type_parameter() {
    for type_params in [vec![], vec!["T", "U"]] {
        let core = file(vec![enum_decl(
            "Option",
            type_params.clone(),
            vec![variant_unit("None")],
        )]);
        let errors = lower(&[core, file(vec![fun("main", vec![])])])
            .expect_err("wrong Option arity must fail");
        let count = type_params.len();
        assert!(
            errors.iter().any(|e| e.message
                == format!(
                    "enum `Option` in scoop.core must have exactly one type parameter, found {count}"
                )),
            "{errors:?}"
        );
    }
}

#[test]
fn duplicate_option_in_core_is_an_error() {
    let option = || {
        enum_decl(
            "Option",
            vec!["T"],
            vec![
                variant_positional("Some", vec![ty_named("T")]),
                variant_unit("None"),
            ],
        )
    };
    let errors = lower(&[
        file(vec![option()]),
        file(vec![option()]),
        file(vec![fun("main", vec![])]),
    ])
    .expect_err("a second core Option must fail");
    // The same run also reports the missing core `Throwable` (M8);
    // this test is about the duplicate's file index.
    let duplicate = errors
        .iter()
        .find(|e| e.message == "duplicate enum `Option`")
        .expect("the second core Option must be diagnosed");
    // The second core file is the error's file.
    assert_eq!(duplicate.file, 1);
}

// --- negative: intrinsics ---

#[test]
fn unknown_intrinsic_is_an_error() {
    let mut core = core_file();
    core.declarations
        .push(intrinsic_fun("reset", "rt_reset", vec![], None));
    let errors =
        lower(&[core, file(vec![fun("main", vec![])])]).expect_err("unknown intrinsic must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown intrinsic `rt_reset`");
    assert_eq!(errors[0].file, 0);
}

#[test]
fn intrinsic_in_user_file_is_an_error() {
    let user = file(vec![
        intrinsic_fun(
            "wipe",
            "rt_write",
            vec![("message", ty_named("String"))],
            None,
        ),
        fun("main", vec![]),
    ]);
    let errors = lower(&[core_file(), user]).expect_err("user `@Intrinsic` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "`@Intrinsic` is only allowed in the core library"
    );
    assert_eq!(errors[0].file, 1);
}

#[test]
fn unsupported_annotation_is_an_error() {
    let mut core = core_file();
    core.declarations.push(Decl::Function(FunctionDecl {
        annotations: vec![ast::Annotation {
            name: ident("NoGC"),
            value: None,
            span: sp(),
        }],
        is_suspend: false,
        is_override: false,
        modifier: ast::MethodModifier::Final,
        name: ident("pure_fn"),
        type_params: vec![],
        params: vec![],
        return_ty: None,
        body: FunctionBody::Block(block(vec![])),
        span: sp(),
    }));
    let errors = lower(&[core, file(vec![fun("main", vec![])])])
        .expect_err("unsupported annotation must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unsupported annotation `@NoGC`");
}

// --- negative: variant construction ---

#[test]
fn unknown_enum_in_qualified_path_is_an_error() {
    let file = file(vec![fun("main", vec![val("c", field(var("Foo"), "Red"))])]);
    let errors = lower_user(file).expect_err("unknown enum must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown variable `Foo`");
}

#[test]
fn unknown_variant_in_qualified_path_is_an_error() {
    let file = file(vec![
        color_decl(),
        fun("main", vec![val("c", field(var("Color"), "Purple"))]),
    ]);
    let errors = lower_user(file).expect_err("unknown variant must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "enum `Color` has no variant `Purple`");
}

#[test]
fn unknown_variant_in_dotted_call_is_an_error() {
    let file = file(vec![
        color_decl(),
        fun("main", vec![val("c", call("Color.Purple", vec![]))]),
    ]);
    let errors = lower_user(file).expect_err("unknown variant must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "enum `Color` has no variant `Purple`");
}

/// Only `Option`'s variants are globally visible (M4 simplification):
/// other enums' variants need the `E.V` prefix.
#[test]
fn bare_non_option_variant_is_an_error() {
    let file = file(vec![
        color_decl(),
        fun(
            "main",
            vec![val("c", var("Red")), val("d", call("Green", vec![]))],
        ),
    ]);
    let errors = lower_user(file).expect_err("bare variants must fail");
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].message, "unknown variable `Red`");
    assert_eq!(errors[1].message, "unknown function `Green`");
}

#[test]
fn variant_arity_is_an_error() {
    let file = file(vec![
        shape_decl(),
        fun(
            "main",
            vec![val("s", call("Shape.Circle", vec![int_lit(1), int_lit(2)]))],
        ),
    ]);
    let errors = lower_user(file).expect_err("too many arguments must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "variant `Circle` of `Shape` takes exactly 1 argument, but 2 were supplied"
    );
}

#[test]
fn variant_missing_argument_without_default_is_an_error() {
    let file = file(vec![
        shape_decl(),
        fun(
            "main",
            vec![val("s", call("Shape.Named", vec![int_lit(1)]))],
        ),
    ]);
    let errors = lower_user(file).expect_err("missing argument must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "variant `Named` of `Shape` takes exactly 2 arguments, but 1 were supplied"
    );
}

#[test]
fn variant_argument_type_mismatch_is_an_error() {
    let file = file(vec![
        shape_decl(),
        fun(
            "main",
            vec![val("s", call("Shape.Circle", vec![str_lit("x")]))],
        ),
    ]);
    let errors = lower_user(file).expect_err("argument mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "argument for field `_1` of `Shape.Circle` must be of type Int, found String"
    );
}

/// A qualified unit variant of a generic enum needs an expected type
/// to take its type arguments from.
#[test]
fn qualified_none_needs_an_expected_type() {
    let file = file(vec![fun(
        "main",
        vec![val("o", field(var("Option"), "None"))],
    )]);
    let errors = lower_user(file).expect_err("unhinted `Option.None` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "cannot infer the type of `None`");
}

#[test]
fn qualified_none_with_expected_type_lowers() {
    let file = file(vec![fun(
        "main",
        vec![val_ty(
            "o",
            Some(ty_nullable(ty_named("Int"))),
            field(var("Option"), "None"),
        )],
    )]);
    let module = lower_user(file).expect("hinted `Option.None` must lower");
    let dump = hir::dump(&module);
    assert!(
        dump.contains("VariantConstruct Option.None<Int> : Option<Int>"),
        "{dump}"
    );
}

#[test]
fn non_unit_variant_without_call_is_an_error() {
    let file = file(vec![fun("main", vec![val("o", var("Some"))])]);
    let errors = lower_user(file).expect_err("bare `Some` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "variant `Some` of `Option` takes arguments; use `Some(...)` to construct it"
    );
}

#[test]
fn generic_enum_annotation_without_type_arguments_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val_ty("o", Some(ty_named("Option")), none())],
    )]);
    let errors = lower_user(file).expect_err("bare generic enum must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "generic enum `Option` requires 1 type argument(s)"
    );
}

#[test]
fn variant_construction_statement_is_not_a_call() {
    let file = file(vec![
        color_decl(),
        fun("main", vec![stmt(field(var("Color"), "Red"))]),
    ]);
    let errors = lower_user(file).expect_err("construction statement must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "statement must be a function call");
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

// --- negative: destructuring declarations ---

#[test]
fn refutable_patterns_in_val_are_an_error() {
    for (target, init) in [
        (pat_pos(&["Some"], vec![pat_bind("x")], None), var("o")),
        (pat_lit(int_lit(0)), var("o")),
        // Nested refutable pattern inside an irrefutable one.
        (
            pat_tuple(
                vec![pat_pos(&["Some"], vec![pat_bind("x")], None), pat_wild()],
                None,
            ),
            tuple_lit(vec![var("o"), int_lit(2)]),
        ),
    ] {
        let file = file(vec![fun(
            "main",
            vec![
                val_ty("o", Some(ty_nullable(ty_named("Int"))), some(int_lit(1))),
                val_pat(false, target, None, init),
            ],
        )]);
        let errors = lower_user(file).expect_err("refutable pattern in val must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "refutable patterns are only allowed in `when`"
        );
    }
}

/// A bare `None` in a `val` target is a (refutable) unit variant
/// pattern, not a binding.
#[test]
fn bare_unit_variant_in_val_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val_ty("o", Some(ty_nullable(ty_named("Int"))), none()),
            val_pat(false, pat_bind("None"), None, var("o")),
        ],
    )]);
    let errors = lower_user(file).expect_err("`val None` must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "refutable patterns are only allowed in `when`"
    );
}

#[test]
fn destructuring_type_mismatch_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![val_pat(
            false,
            pat_tuple(vec![pat_bind("x"), pat_bind("y")], None),
            None,
            tuple_lit(vec![int_lit(1)]),
        )],
    )]);
    let errors = lower_user(file).expect_err("pattern/type mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "pattern has 2 element(s), but tuple of type (Int) has 1"
    );
}

#[test]
fn literal_pattern_type_mismatch_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val("t", tuple_lit(vec![int_lit(1), str_lit("s")])),
            when_stmt(
                var("t"),
                vec![
                    arm(
                        pat_tuple(vec![pat_lit(str_lit("x")), pat_bind("s")], None),
                        None,
                        vec![],
                    ),
                    arm(pat_wild(), None, vec![]),
                ],
                None,
            ),
        ],
    )]);
    let errors = lower_user(file).expect_err("literal mismatch must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "literal pattern of type String cannot match Int"
    );
}

#[test]
fn non_literal_literal_pattern_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val("t", tuple_lit(vec![int_lit(1)])),
            when_stmt(
                var("t"),
                vec![
                    arm(
                        pat_tuple(vec![pat_lit(call("f", vec![]))], None),
                        None,
                        vec![],
                    ),
                    arm(pat_wild(), None, vec![]),
                ],
                None,
            ),
        ],
    )]);
    let errors = lower_user(file).expect_err("non-literal pattern must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "expected a literal pattern");
}

#[test]
fn duplicate_binding_in_pattern_is_an_error() {
    let file = file(vec![fun(
        "main",
        vec![
            val("t", tuple_lit(vec![int_lit(1), int_lit(2)])),
            when_stmt(
                var("t"),
                vec![arm(
                    pat_tuple(vec![pat_bind("x"), pat_bind("x")], None),
                    None,
                    vec![],
                )],
                None,
            ),
        ],
    )]);
    let errors = lower_user(file).expect_err("duplicate binding must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "`x` is already declared in this scope");
}
