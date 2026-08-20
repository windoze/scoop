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
    let expected = include_str!("snapshots/enum_declaration_all_variant_forms.hir.txt");
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
        FunctionKind::Intrinsic(_)
        | FunctionKind::Extern(_)
        | FunctionKind::DerivedEquality
        | FunctionKind::Abstract { .. }
        | FunctionKind::InitializationEnsure => {
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
    let expression = module.export_default_sources[source].declared().unwrap().0;
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
                annotations: Vec::new(),
                name: ident("Named"),
                kind: VariantDeclKind::Named(vec![VariantFieldDecl {
                    annotations: Vec::new(),
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
