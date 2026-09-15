use super::*;

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
                val(
                    "n",
                    source_call(
                        "Shape.Named",
                        vec![
                            named_argument("w", int_lit(2)),
                            named_argument("h", int_lit(3)),
                        ],
                    ),
                ),
            ],
        ),
    ]);
    let module = lower_user(file).expect("enum program must lower");
    let expected = r#"Module
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
      VariantConstruct Color.Red : Color
    val local1
      IntegerLiteral 1 : Int
    val local2
      Local $argument.0 : Int
    val local3
      VariantConstruct Shape.Circle : Shape
        Local $parameter._1 : Int
    val local4
      IntegerLiteral 0 : Int
    val local5
      VariantConstruct Shape.WithDefault : Shape
        Local $parameter.d : Int
    val local6
      IntegerLiteral 2 : Int
    val local7
      IntegerLiteral 3 : Int
    val local8
      Local $argument.0 : Int
    val local9
      Local $argument.1 : Int
    val local10
      VariantConstruct Shape.Named : Shape
        Local $parameter.w : Int
        Local $parameter.h : Int
  output executable main
"#;
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
    let body = match &module.functions[module.entry()].kind {
        FunctionKind::User(body) => body,
        FunctionKind::Intrinsic(_) | FunctionKind::Extern(_) | FunctionKind::DerivedEquality => {
            panic!("main is a user function")
        }
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
    assert!(
        dump.contains("MethodCall Option.equals <derived> : Boolean"),
        "{dump}"
    );
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
fn non_literal_variant_default_is_typed_at_the_definition() {
    let file = file(vec![
        enum_decl(
            "Shape",
            vec![],
            vec![variant_constructor(
                "WithDefault",
                vec![("d", ty_named("Int"), Some(call("f", vec![])))],
            )],
        ),
        fun_expr("f", vec![], vec![], Some(ty_named("Int")), int_lit(9)),
        fun(
            "main",
            vec![val("shape", call("Shape.WithDefault", vec![]))],
        ),
    ]);
    let module = lower_user(file).expect("a typed call is a valid variant default");
    let dump = hir::dump(&module);
    assert!(dump.contains("Call f : Int"), "{dump}");
    assert!(
        dump.contains("VariantConstruct Shape.WithDefault : Shape"),
        "{dump}"
    );
}

#[test]
fn variant_parameter_interface_keeps_its_checked_owner_identity() {
    let file = file(vec![
        enum_decl(
            "Shape",
            vec![],
            vec![variant_constructor(
                "WithDefault",
                vec![("d", ty_named("Int"), Some(int_lit(9)))],
            )],
        ),
        fun("main", vec![]),
    ]);
    let module = lower_user(file).expect("the variant default must lower");
    let shape = module
        .enums
        .iter()
        .find_map(|(id, declaration)| (declaration.name == "Shape").then_some(id))
        .expect("Shape enum");
    let variant = hir::EnumVariantRef::checked(&module.enums, shape, 0)
        .expect("Shape.WithDefault checked identity");
    let interface = module
        .source_parameter_interfaces
        .iter()
        .find(|interface| interface.owner == hir::ExportParameterOwner::VariantConstructor(variant))
        .expect("variant constructor parameter protocol");
    assert_eq!(interface.parameters.len(), 1);
    let hir::ExportParameterCalling::Default { source, .. } = interface.parameters[0].calling
    else {
        panic!("variant parameter default");
    };
    let expression = module.export_default_sources[source].expression;
    assert_eq!(
        module.export_default_exprs[expression].definition_root,
        hir::LexicalDefinitionRoot::VariantConstructor(variant)
    );
    assert_eq!(
        module.enums[variant.enumeration()].variants[variant.local_index() as usize].name,
        "WithDefault"
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
        "default value of parameter `d` in `Shape.WithDefault` must be of type Int, found String"
    );
}

#[test]
fn named_variant_default_is_an_error() {
    // The parser only produces defaults on constructor-style variants;
    // HIR rejects the shape too.
    let file = file(vec![
        Decl::Enum(ast::EnumDecl {
            annotations: vec![],
            visibility: ast::VisibilitySyntax::Omitted,
            name: ident("Shape"),
            type_params: vec![],
            methods: vec![],
            interfaces: vec![],
            where_clause: None,
            variants: vec![VariantDecl {
                name: ident("Named"),
                kind: VariantDeclKind::Named(vec![VariantFieldDecl {
                    name: ident("w"),
                    ty: ty_named("Int"),
                    syntax: ast::ParameterSyntax::Default {
                        expression: int_lit(0),
                        equals_span: sp(),
                    },
                    span: sp(),
                }]),
                span: sp(),
            }],
            properties: Vec::new(),
            nested: Vec::new(),
            companion: None,
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
