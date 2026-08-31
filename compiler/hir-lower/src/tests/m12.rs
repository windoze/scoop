//! M12 typed annotations, lexical safety and `@NoGC` verification.

use scoop_ast as ast;
use scoop_hir as hir;

use super::*;

fn marker(name: &str) -> ast::Annotation {
    ast::Annotation {
        name: ident(name),
        args: Vec::new(),
        span: sp(),
    }
}

fn string_annotation(name: &str, value: &str) -> ast::Annotation {
    ast::Annotation {
        name: ident(name),
        args: vec![ast::AnnotationArg {
            name: None,
            value: ast::AnnotationLiteral::String(value.to_string()),
            span: sp(),
        }],
        span: sp(),
    }
}

fn annotate(mut decl: Decl, annotations: Vec<ast::Annotation>) -> Decl {
    let Decl::Function(function) = &mut decl else {
        panic!("expected function declaration");
    };
    function.annotations = annotations;
    decl
}

fn annotate_method(mut method: FunctionDecl, annotations: Vec<ast::Annotation>) -> FunctionDecl {
    method.annotations = annotations;
    method
}

fn annotate_struct(mut decl: Decl, annotations: Vec<ast::Annotation>) -> Decl {
    let Decl::Struct(strukt) = &mut decl else {
        panic!("expected struct declaration");
    };
    strukt.annotations = annotations;
    decl
}

fn annotate_enum(mut decl: Decl, annotations: Vec<ast::Annotation>) -> Decl {
    let Decl::Enum(enumeration) = &mut decl else {
        panic!("expected enum declaration");
    };
    enumeration.annotations = annotations;
    decl
}

fn c_layout(aligned: i64, packed: i64) -> ast::Annotation {
    ast::Annotation {
        name: ident("CLayout"),
        args: vec![
            ast::AnnotationArg {
                name: Some(ident("aligned")),
                value: ast::AnnotationLiteral::Int(aligned),
                span: sp(),
            },
            ast::AnnotationArg {
                name: Some(ident("packed")),
                value: ast::AnnotationLiteral::Int(packed),
                span: sp(),
            },
        ],
        span: sp(),
    }
}

fn extern_annotation(lib: &str, name: &str, abi: &str) -> ast::Annotation {
    ast::Annotation {
        name: ident("Extern"),
        args: [("lib", lib), ("name", name), ("abi", abi)]
            .into_iter()
            .map(|(parameter, value)| ast::AnnotationArg {
                name: Some(ident(parameter)),
                value: ast::AnnotationLiteral::String(value.to_string()),
                span: sp(),
            })
            .collect(),
        span: sp(),
    }
}

fn extern_fun(
    name: &str,
    params: Vec<(&str, ast::TypeRef)>,
    return_ty: Option<ast::TypeRef>,
    annotation: ast::Annotation,
) -> Decl {
    let mut declaration = fun_sig(name, vec![], params, return_ty, vec![]);
    let Decl::Function(function) = &mut declaration else {
        unreachable!()
    };
    function.annotations = vec![annotation];
    function.body = ast::FunctionBody::None;
    declaration
}

fn with_kind(mut decl: Decl, kind: ast::TypeParamKindBound) -> Decl {
    let type_params = match &mut decl {
        Decl::Function(decl) => &mut decl.type_params,
        Decl::Struct(decl) => &mut decl.type_params,
        Decl::Enum(decl) => &mut decl.type_params,
        Decl::Interface(decl) => &mut decl.type_params,
        Decl::Class(_) => panic!("classes have no M12 type parameters"),
        Decl::Global(_) => panic!("globals have no type parameters"),
    };
    type_params[0].kind_bound = Some(kind);
    decl
}

fn safety_block(mode: ast::SafetyMode, statements: Vec<Statement>) -> Statement {
    Statement {
        kind: StatementKind::SafetyBlock {
            mode,
            block: block(statements),
        },
        span: sp(),
    }
}

fn messages(decls: Vec<Decl>) -> Vec<String> {
    lower_user(file(decls))
        .expect_err("program must be rejected")
        .into_iter()
        .map(|error| error.message)
        .collect()
}

#[test]
fn no_gc_value_type_contract_is_checked_in_hir_after_specialization() {
    let good = annotate_struct(
        struct_decl(
            "NativePair",
            vec![("x", ty_named("Int")), ("y", ty_named("UInt"))],
        ),
        vec![marker("NoGC")],
    );
    let good_enum = annotate_enum(
        enum_decl(
            "NativeResult",
            vec![],
            vec![
                variant_positional("Value", vec![ty_named("Int")]),
                variant_unit("Empty"),
            ],
        ),
        vec![marker("NoGC")],
    );
    let output = lower_user_output(file(vec![
        good,
        good_enum,
        fun_sig(
            "consumeResult",
            vec![],
            vec![("result", ty_named("NativeResult"))],
            None,
            vec![],
        ),
        fun("main", vec![]),
    ]))
    .expect("a concrete GC-free struct satisfies @NoGC");
    let (_, native_pair) = output
        .local
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "NativePair")
        .expect("NativePair concrete definition");
    assert!(native_pair.gc_free);
    let (_, native_result) = output
        .local
        .enums
        .iter()
        .find(|(_, declaration)| declaration.name == "NativeResult")
        .expect("NativeResult concrete definition");
    assert!(native_result.gc_free);
    assert!(native_result.variants.iter().all(|variant| variant.gc_free));

    let bad_struct = annotate_struct(
        struct_decl("BadNativeValue", vec![("text", ty_named("String"))]),
        vec![marker("NoGC")],
    );
    let bad_enum = annotate_enum(
        enum_decl(
            "BadNativeEnum",
            vec![],
            vec![
                variant_positional("Text", vec![ty_named("String")]),
                variant_unit("Empty"),
            ],
        ),
        vec![marker("NoGC")],
    );
    let errors = messages(vec![bad_struct, bad_enum, fun("main", vec![])]);
    assert!(errors.iter().any(|message| {
        message.contains("`@NoGC` struct specialization `BadNativeValue`")
            && message.contains("contains a ref type")
    }));
    assert!(errors.iter().any(|message| {
        message.contains("`@NoGC` enum specialization `BadNativeEnum`")
            && message.contains("contains a ref type")
    }));
}

#[test]
fn no_gc_generic_type_waits_for_concrete_arguments() {
    let generic = annotate_struct(
        generic_struct_decl("NativeBox", vec!["T"], vec![("value", ty_named("T"))]),
        vec![marker("NoGC")],
    );
    lower_user(file(vec![generic.clone(), fun("main", vec![])]))
        .expect("an uninstantiated generic has no GC-free boolean result");

    let errors = messages(vec![
        generic,
        fun_sig(
            "consume",
            vec![],
            vec![("box", ty_generic("NativeBox", vec![ty_named("String")]))],
            None,
            vec![],
        ),
        fun("main", vec![]),
    ]);
    assert!(errors.iter().any(|message| {
        message.contains("`@NoGC` struct specialization `NativeBox<String>`")
            && message.contains("contains a ref type")
    }));
}

#[test]
fn pointer_core_normalizes_construction_memory_ops_and_layout_queries() {
    let pointer_ty = ty_generic("Ptr", vec![ty_named("Int")]);
    let program = file(vec![fun(
        "main",
        vec![
            val_ty(
                "raw",
                Some(ty_named("UInt")),
                typed_call("sizeOf", vec![ty_named("Int")], vec![]),
            ),
            safety_block(
                ast::SafetyMode::Unsafe,
                vec![
                    val_ty(
                        "p",
                        Some(pointer_ty.clone()),
                        typed_call("Ptr", vec![ty_named("Int")], vec![var("raw")]),
                    ),
                    val("local", int_lit(7)),
                    val(
                        "address",
                        typed_call("addressOf", vec![ty_named("Int")], vec![var("local")]),
                    ),
                    val("next", binary(ast::BinOp::Add, var("p"), int_lit(2))),
                    val("loaded", method_call(var("p"), "load", vec![])),
                    stmt(method_call(var("p"), "store", vec![int_lit(9)])),
                    val(
                        "casted",
                        typed_method_call(var("p"), "cast", vec![ty_named("Boolean")], vec![]),
                    ),
                    val_ty(
                        "alignment",
                        Some(ty_named("UInt")),
                        typed_call("alignOf", vec![ty_named("Int")], vec![]),
                    ),
                ],
            ),
        ],
    )]);
    let module = lower_user(program).expect("typed pointer operations must lower");
    let dump = hir::dump(&module);
    for node in [
        "PtrFromUInt",
        "AddressOf local",
        "PtrOffset subtract=false",
        "PtrLoad",
        "PtrStore",
        "PtrCast",
        "SizeOf Int",
        "AlignOf Int",
    ] {
        assert!(dump.contains(node), "missing `{node}` in:\n{dump}");
    }
    assert!(module.types.iter().any(
        |(_, ty)| matches!(ty, hir::Type::Ptr(pointee) if module.types[*pointee] == hir::Type::Int)
    ));
}

#[test]
fn pointer_operations_enforce_unsafe_lvalue_and_gc_free_constraints() {
    let errors = messages(vec![fun(
        "main",
        vec![val(
            "p",
            typed_call(
                "Ptr",
                vec![ty_named("Int")],
                vec![typed_call("sizeOf", vec![ty_named("Int")], vec![])],
            ),
        )],
    )]);
    assert!(errors.iter().any(|message| {
        message.contains("constructing `Ptr` from a raw integer requires an unsafe context")
    }));

    let errors = messages(vec![fun(
        "main",
        vec![safety_block(
            ast::SafetyMode::Unsafe,
            vec![stmt(call("addressOf", vec![int_lit(1)]))],
        )],
    )]);
    assert!(
        errors
            .iter()
            .any(|message| message.contains("`addressOf` argument must be an addressable"))
    );

    let errors = messages(vec![
        struct_decl("TextBox", vec![("value", ty_named("String"))]),
        fun_sig(
            "take",
            vec![],
            vec![("p", ty_generic("Ptr", vec![ty_named("TextBox")]))],
            None,
            vec![],
        ),
        fun("main", vec![]),
    ]);
    assert!(
        errors
            .iter()
            .any(|message| message.contains("`Ptr` pointee must be GC-free"))
    );
}

#[test]
fn c_layout_accepts_nested_pointer_callback_and_nullable_pointer_fields() {
    let inner = annotate_struct(
        struct_decl(
            "CInner",
            vec![("flag", ty_named("Boolean")), ("value", ty_named("Int"))],
        ),
        vec![c_layout(8, 1)],
    );
    let callback = ty_generic(
        "FunPtr",
        vec![ty_function(false, vec![ty_named("Int")], ty_named("Unit"))],
    );
    let outer = annotate_struct(
        struct_decl(
            "COuter",
            vec![
                ("inner", ty_named("CInner")),
                ("pointer", ty_generic("Ptr", vec![ty_named("Int")])),
                (
                    "optional",
                    ty_nullable(ty_generic("Ptr", vec![ty_named("Boolean")])),
                ),
                ("callback", callback),
            ],
        ),
        vec![c_layout(16, 8)],
    );
    lower_user(file(vec![inner, outer, fun("main", vec![])]))
        .expect("the complete M12 C field set must be C-FFI-safe");
}

#[test]
fn c_layout_reports_complete_nested_field_paths_and_deferred_generic_failures() {
    let invalid_inner = annotate_struct(
        struct_decl("BadInner", vec![("text", ty_named("String"))]),
        vec![c_layout(0, 0)],
    );
    let invalid_outer = annotate_struct(
        struct_decl("BadOuter", vec![("inner", ty_named("BadInner"))]),
        vec![c_layout(0, 0)],
    );
    let plain = struct_decl("Plain", vec![("value", ty_named("Int"))]);
    let contains_plain = annotate_struct(
        struct_decl("ContainsPlain", vec![("plain", ty_named("Plain"))]),
        vec![c_layout(0, 0)],
    );
    let generic = annotate_struct(
        generic_struct_decl("CBox", vec!["T"], vec![("value", ty_named("T"))]),
        vec![c_layout(0, 0)],
    );
    let messages = messages(vec![
        invalid_inner,
        invalid_outer,
        plain,
        contains_plain,
        generic,
        fun_sig(
            "consume",
            vec![],
            vec![("box", ty_generic("CBox", vec![ty_named("String")]))],
            None,
            vec![],
        ),
        fun("main", vec![]),
    ]);
    assert!(messages.iter().any(|message| {
        message.contains("BadOuter.inner.text") && message.contains("ref type `String`")
    }));
    assert!(messages.iter().any(|message| {
        message.contains("ContainsPlain.plain")
            && message.contains("ordinary struct `Plain` has no stable C layout")
    }));
    assert!(messages.iter().any(|message| {
        message.contains("CBox<String>.value") && message.contains("ref type `String`")
    }));
}

#[test]
fn c_layout_rejects_empty_structs_and_managed_fun_ptr_signatures() {
    let empty = annotate_struct(struct_decl("Empty", vec![]), vec![c_layout(0, 0)]);
    let signature = ty_function(false, vec![ty_named("String")], ty_named("Int"));
    let fun_ptr = ty_generic("FunPtr", vec![signature.clone()]);
    let errors = messages(vec![
        empty,
        fun(
            "main",
            vec![val_ty(
                "callback",
                Some(fun_ptr),
                typed_call("FunPtr", vec![signature], vec![]),
            )],
        ),
    ]);
    assert!(errors.iter().any(|message| {
        message.contains("`@CLayout` struct `Empty` must declare at least one field")
    }));
    assert!(errors.iter().any(|message| {
        message.contains("`FunPtr` signature is not C-FFI-safe")
            && message.contains("parameter1")
            && message.contains("ref type `String`")
    }));
}

#[test]
fn fun_ptr_null_and_native_function_reference_are_distinct_from_managed_values() {
    let signature = ty_function(false, vec![ty_named("Int")], ty_named("Int"));
    let fun_ptr = ty_generic("FunPtr", vec![signature.clone()]);
    let reference = ast::Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: None,
        name: ident("increment"),
        span: sp(),
    };
    let callback = annotate(
        fun_expr(
            "increment",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            binary(ast::BinOp::Add, var("value"), int_lit(1)),
        ),
        vec![marker("NoGC")],
    );
    let module = lower_user(file(vec![
        callback,
        fun(
            "main",
            vec![
                val_ty("callback", Some(fun_ptr.clone()), reference),
                val_ty(
                    "empty",
                    Some(fun_ptr),
                    typed_call("FunPtr", vec![signature], vec![]),
                ),
            ],
        ),
    ]))
    .expect("native function address and null FunPtr must lower");
    let dump = hir::dump(&module);
    assert!(dump.contains("FunctionAddress increment"));
    assert!(dump.contains("FunPtrNull"));
    assert_eq!(module.callable_references.len(), 0);

    let reference = ast::Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: None,
        name: ident("managed"),
        span: sp(),
    };
    let errors = messages(vec![
        fun_expr(
            "managed",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            var("value"),
        ),
        fun(
            "main",
            vec![val_ty(
                "callback",
                Some(ty_generic(
                    "FunPtr",
                    vec![ty_function(false, vec![ty_named("Int")], ty_named("Int"))],
                )),
                reference,
            )],
        ),
    ]);
    assert!(
        errors
            .iter()
            .any(|message| message.contains("no eligible `@NoGC` top-level function"))
    );
}

#[test]
fn kind_bounds_are_typed_on_all_generic_hir_declarations() {
    let function = with_kind(
        fun_expr(
            "identity",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(ty_named("T")),
            var("value"),
        ),
        ast::TypeParamKindBound::Value,
    );
    let strukt = with_kind(
        generic_struct_decl("RefBox", vec!["T"], vec![("value", ty_named("T"))]),
        ast::TypeParamKindBound::Ref,
    );
    let enumeration = with_kind(
        enum_decl("Choice", vec!["T"], vec![variant_unit("None")]),
        ast::TypeParamKindBound::Value,
    );
    let interface = with_kind(
        generic_interface_decl(
            "Source",
            vec![(ast::Variance::Out, "T")],
            vec![bodyless_method(false, "get", vec![], Some(ty_named("T")))],
        ),
        ast::TypeParamKindBound::Ref,
    );
    let module = lower_user(file(vec![
        function,
        strukt,
        enumeration,
        interface,
        fun("main", vec![]),
    ]))
    .expect("kind bounds must lower to typed HIR");

    let identity = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "identity")
        .unwrap()
        .1;
    assert_eq!(identity.type_params[0].kind, hir::TypeParamKind::Value);
    assert_eq!(
        module
            .structs
            .iter()
            .find(|(_, decl)| decl.name == "RefBox")
            .unwrap()
            .1
            .type_params[0]
            .kind,
        hir::TypeParamKind::Ref
    );
    assert_eq!(
        module
            .enums
            .iter()
            .find(|(_, decl)| decl.name == "Choice")
            .unwrap()
            .1
            .type_params[0]
            .kind,
        hir::TypeParamKind::Value
    );
    assert_eq!(
        module
            .interfaces
            .iter()
            .find(|(_, decl)| decl.name == "Source")
            .unwrap()
            .1
            .type_params[0]
            .kind,
        hir::TypeParamKind::Ref
    );
    let dump = hir::dump(&module);
    assert!(dump.contains("struct RefBox<T : ref>"));
    assert!(dump.contains("enum Choice<T : value>"));
    assert!(dump.contains("interface Source<out T : ref>"));
    assert!(dump.contains("fun identity<T : value>"));
}

#[test]
fn kind_bounds_check_concrete_function_and_struct_instantiations() {
    let value_identity = with_kind(
        fun_expr(
            "valueIdentity",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(ty_named("T")),
            var("value"),
        ),
        ast::TypeParamKindBound::Value,
    );
    let ref_identity = with_kind(
        fun_expr(
            "refIdentity",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(ty_named("T")),
            var("value"),
        ),
        ast::TypeParamKindBound::Ref,
    );
    let ref_box = with_kind(
        generic_struct_decl("RefBox", vec!["T"], vec![("value", ty_named("T"))]),
        ast::TypeParamKindBound::Ref,
    );
    let wrapper = struct_decl("Wrapper", vec![("text", ty_named("String"))]);

    lower_user(file(vec![
        value_identity.clone(),
        ref_identity.clone(),
        ref_box.clone(),
        wrapper.clone(),
        fun(
            "main",
            vec![
                val(
                    "value",
                    call(
                        "valueIdentity",
                        vec![struct_init("Wrapper", vec![str_lit("managed field")])],
                    ),
                ),
                val("reference", call("refIdentity", vec![str_lit("text")])),
                val("boxed", struct_init("RefBox", vec![str_lit("text")])),
            ],
        ),
    ]))
    .expect("value/ref kinds are independent from the recursive GC-free property");

    let errors = messages(vec![
        value_identity,
        ref_identity,
        ref_box,
        wrapper,
        fun(
            "main",
            vec![
                val("badValue", call("valueIdentity", vec![str_lit("text")])),
                val("badRef", call("refIdentity", vec![int_lit(1)])),
                val("badBox", struct_init("RefBox", vec![int_lit(1)])),
            ],
        ),
    ]);
    assert!(errors.iter().any(|message| message
        == "type argument `String` for `T` of function `valueIdentity` must satisfy `value`"));
    assert!(errors.iter().any(|message| message
        == "type argument `Int` for `T` of function `refIdentity` must satisfy `ref`"));
    assert!(
        errors.iter().any(|message| message
            == "type argument `Int` for `T` of struct `RefBox` must satisfy `ref`")
    );
}

#[test]
fn explicit_type_arguments_bind_functions_constructors_variants_and_method_suffixes() {
    let identity = fun_expr(
        "identity",
        vec!["T"],
        vec![("value", ty_named("T"))],
        Some(ty_named("T")),
        var("value"),
    );
    let box_decl = generic_struct_decl("Box", vec!["T"], vec![("value", ty_named("T"))]);
    let choice = enum_decl(
        "Choice",
        vec!["T"],
        vec![variant_positional("Some", vec![ty_named("T")])],
    );
    let mut convert = method_expr(
        "convert",
        vec![("value", ty_named("U"))],
        Some(ty_named("U")),
        var("value"),
    );
    convert.type_params = vec![type_param("U")];
    let holder = generic_struct_decl_full(
        "Holder",
        vec!["T"],
        vec![("value", ty_named("T"))],
        vec![],
        vec![convert],
    );
    let module = lower_user(file(vec![
        identity,
        box_decl,
        choice,
        holder,
        fun(
            "main",
            vec![
                val(
                    "function",
                    typed_call("identity", vec![ty_named("Int")], vec![int_lit(1)]),
                ),
                val(
                    "constructor",
                    typed_call("Box", vec![ty_named("String")], vec![str_lit("box")]),
                ),
                val(
                    "variant",
                    typed_method_call(
                        var("Choice"),
                        "Some",
                        vec![ty_named("Int")],
                        vec![int_lit(2)],
                    ),
                ),
                val("holder", struct_init("Holder", vec![int_lit(3)])),
                val(
                    "method",
                    typed_method_call(
                        var("holder"),
                        "convert",
                        vec![ty_named("String")],
                        vec![str_lit("method")],
                    ),
                ),
            ],
        ),
    ]))
    .expect("complete explicit type argument lists must lower");

    let dump = hir::dump(&module);
    assert!(dump.contains("Call identity<Int> : Int"), "{dump}");
    assert!(dump.contains("StructInit Box : Box<String>"), "{dump}");
    assert!(
        dump.contains("VariantConstruct Choice.Some<Int> : Choice<Int>"),
        "{dump}"
    );
    assert!(
        dump.contains("MethodCall Holder.convert : String")
            && dump.contains("instance Holder.convert<Int, String>"),
        "{dump}"
    );
}

#[test]
fn explicit_type_arguments_filter_overloads_and_report_complete_list_errors() {
    let generic = fun_expr(
        "pick",
        vec!["T"],
        vec![("value", ty_named("T"))],
        Some(ty_named("T")),
        var("value"),
    );
    let concrete = fun_expr(
        "pick",
        vec![],
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        var("value"),
    );
    let module = lower_user(file(vec![
        generic.clone(),
        concrete.clone(),
        fun(
            "main",
            vec![val(
                "picked",
                typed_call("pick", vec![ty_named("String")], vec![str_lit("value")]),
            )],
        ),
    ]))
    .expect("explicit arguments must exclude overloads with another generic arity");
    assert!(hir::dump(&module).contains("Call pick<String> : String"));

    let errors = messages(vec![
        generic,
        concrete,
        fun(
            "main",
            vec![stmt(typed_call(
                "pick",
                vec![ty_named("Int"), ty_named("String")],
                vec![int_lit(1)],
            ))],
        ),
    ]);
    assert_eq!(
        errors,
        ["no overload of `pick` accepts 2 explicit type argument(s)"]
    );
}

#[test]
fn unconstrained_type_parameters_do_not_satisfy_a_stronger_kind() {
    let ref_box = with_kind(
        generic_struct_decl("RefBox", vec!["T"], vec![("value", ty_named("T"))]),
        ast::TypeParamKindBound::Ref,
    );
    let bad = fun_sig(
        "bad",
        vec!["U"],
        vec![("box", ty_generic("RefBox", vec![ty_named("U")]))],
        None,
        vec![],
    );
    let errors = messages(vec![ref_box.clone(), bad, fun("main", vec![])]);
    assert!(errors.iter().any(
        |message| message == "type argument `U` for `T` of struct `RefBox` must satisfy `ref`"
    ));

    let good = with_kind(
        fun_sig(
            "good",
            vec!["U"],
            vec![("box", ty_generic("RefBox", vec![ty_named("U")]))],
            None,
            vec![],
        ),
        ast::TypeParamKindBound::Ref,
    );
    lower_user(file(vec![ref_box, good, fun("main", vec![])]))
        .expect("a matching outer kind bound proves the nested application");
}

#[test]
fn ref_bound_parameters_have_reference_semantics_inside_the_definition() {
    let same = with_kind(
        fun_expr(
            "same",
            vec!["T"],
            vec![("left", ty_named("T")), ("right", ty_named("T"))],
            Some(ty_named("Boolean")),
            binary(ast::BinOp::RefEq, var("left"), var("right")),
        ),
        ast::TypeParamKindBound::Ref,
    );
    lower_user(file(vec![
        same,
        fun(
            "main",
            vec![val(
                "result",
                call("same", vec![str_lit("left"), str_lit("right")]),
            )],
        ),
    ]))
    .expect("a `T : ref` parameter must support reference-only operations");
}

#[test]
fn overloaded_kind_failure_keeps_the_precise_bound_diagnostic() {
    let constrained = with_kind(
        fun_expr(
            "choose",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(ty_named("T")),
            var("value"),
        ),
        ast::TypeParamKindBound::Ref,
    );
    let other = fun_expr(
        "choose",
        vec![],
        vec![("left", ty_named("Int")), ("right", ty_named("Int"))],
        Some(ty_named("Int")),
        var("left"),
    );
    let errors = messages(vec![
        constrained,
        other,
        fun(
            "main",
            vec![val("result", call("choose", vec![int_lit(1)]))],
        ),
    ]);
    assert_eq!(
        errors,
        ["type argument `Int` for `T` of function `choose` must satisfy `ref`"]
    );
}

#[test]
fn lowers_function_and_struct_annotations_to_typed_hir() {
    let no_gc = annotate(
        fun_expr(
            "addOne",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            Expr::Binary {
                op: ast::BinOp::Add,
                lhs: Box::new(var("value")),
                rhs: Box::new(int_lit(1)),
                span: sp(),
            },
        ),
        vec![marker("NoGC"), marker("Unsafe")],
    );
    let mut layout = struct_decl("CPoint", vec![("x", ty_named("Int"))]);
    let Decl::Struct(layout_decl) = &mut layout else {
        unreachable!()
    };
    layout_decl.annotations = vec![ast::Annotation {
        name: ident("CLayout"),
        args: vec![
            ast::AnnotationArg {
                name: Some(ident("aligned")),
                value: ast::AnnotationLiteral::Int(8),
                span: sp(),
            },
            ast::AnnotationArg {
                name: Some(ident("packed")),
                value: ast::AnnotationLiteral::Int(1),
                span: sp(),
            },
        ],
        span: sp(),
    }];
    let module = lower_user(file(vec![layout, no_gc, fun("main", vec![])]))
        .expect("typed annotations must lower");
    let function = module
        .functions
        .iter()
        .map(|(_, function)| function)
        .find(|function| function.name == "addOne")
        .expect("function exists");
    assert_eq!(function.attributes.safety, hir::Safety::Unsafe);
    assert_eq!(function.attributes.gc_effect, hir::GcEffect::NoGc);
    assert_eq!(
        module
            .structs
            .iter()
            .find(|(_, decl)| decl.name == "CPoint")
            .unwrap()
            .1
            .attributes
            .c_layout,
        Some(hir::CLayout {
            aligned: 8,
            packed: 1
        })
    );
    let dump = hir::dump(&module);
    assert!(dump.contains("struct CPoint <c-layout aligned=8 packed=1>"));
    assert!(dump.contains("fun addOne(value: Int): Int <unsafe no-gc cdecl>"));
}

#[test]
fn annotation_schema_target_and_coexistence_are_checked_in_hir() {
    let duplicate = annotate(fun("f", vec![]), vec![marker("NoGC"), marker("NoGC")]);
    let errors = messages(vec![duplicate, fun("main", vec![])]);
    assert!(
        errors
            .iter()
            .any(|message| message == "annotation `@NoGC` must not be repeated")
    );

    let both = annotate(fun("f", vec![]), vec![marker("Safe"), marker("Unsafe")]);
    let errors = messages(vec![both, fun("main", vec![])]);
    assert!(
        errors
            .iter()
            .any(|message| message == "`@Safe` and `@Unsafe` cannot be combined")
    );

    let calling = annotate(
        fun("f", vec![]),
        vec![string_annotation("CallingConvention", "stdcall")],
    );
    let errors = messages(vec![calling, fun("main", vec![])]);
    assert!(
        errors
            .iter()
            .any(|message| message.contains("supports only `cdecl`"))
    );
    assert!(
        errors
            .iter()
            .any(|message| message.contains("requires `@NoGC`"))
    );
}

#[test]
fn no_gc_and_calling_convention_reject_suspend_functions() {
    let mut function = annotate(suspend_fun("work", vec![]), vec![marker("NoGC")]);
    let Decl::Function(decl) = &mut function else {
        unreachable!()
    };
    decl.annotations
        .push(string_annotation("CallingConvention", "cdecl"));
    let errors = messages(vec![function, fun("main", vec![])]);
    assert!(
        errors
            .iter()
            .any(|message| message == "`@NoGC` cannot be used on a suspend function")
    );
    assert!(
        errors
            .iter()
            .any(|message| message == "`@CallingConvention` cannot be used on a suspend function")
    );
}

#[test]
fn intrinsic_body_rule_is_owned_by_hir() {
    let mut core = core_file();
    core.declarations.push(annotate(
        fun("badIntrinsic", vec![]),
        vec![string_annotation("Intrinsic", "rt_gc_collect")],
    ));
    let errors =
        lower(&[core, file(vec![fun("main", vec![])])]).expect_err("intrinsic body must fail");
    assert!(errors
        .iter()
        .any(|error| error.message == "`@Intrinsic` functions must not have a body (spec 13.1)"));
}

#[test]
fn unsafe_calls_follow_the_lexical_safety_stack() {
    let dangerous = annotate(fun("dangerous", vec![]), vec![marker("Unsafe")]);
    let errors = messages(vec![
        dangerous.clone(),
        fun("main", vec![stmt(call("dangerous", vec![]))]),
    ]);
    assert!(errors[0].contains("may only be called from an unsafe context"));

    lower_user(file(vec![
        dangerous.clone(),
        fun(
            "main",
            vec![safety_block(
                ast::SafetyMode::Unsafe,
                vec![stmt(call("dangerous", vec![]))],
            )],
        ),
    ]))
    .expect("unsafe block authorizes the call");

    let errors = messages(vec![
        dangerous,
        fun(
            "main",
            vec![safety_block(
                ast::SafetyMode::Unsafe,
                vec![safety_block(
                    ast::SafetyMode::Safe,
                    vec![stmt(call("dangerous", vec![]))],
                )],
            )],
        ),
    ]);
    assert!(errors[0].contains("may only be called from an unsafe context"));
}

#[test]
fn unsafe_function_body_starts_in_unsafe_context() {
    let leaf = annotate(fun("leaf", vec![]), vec![marker("Unsafe")]);
    let caller = annotate(
        fun("caller", vec![stmt(call("leaf", vec![]))]),
        vec![marker("Unsafe")],
    );
    lower_user(file(vec![leaf, caller, fun("main", vec![])]))
        .expect("unsafe body may call unsafe function");
}

#[test]
fn managed_callable_reference_cannot_erase_unsafe_effect() {
    let leaf = annotate(fun("leaf", vec![]), vec![marker("Unsafe")]);
    let reference = Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: None,
        name: ident("leaf"),
        span: sp(),
    };
    let errors = messages(vec![leaf, fun("main", vec![val("f", reference)])]);
    assert!(errors[0].contains("safety is not part of function-type identity"));
}

#[test]
fn no_gc_accepts_value_only_call_graphs_and_recursion() {
    let leaf = annotate(
        fun_expr(
            "leaf",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            var("x"),
        ),
        vec![marker("NoGC")],
    );
    let recursive = annotate(
        fun_sig(
            "recursive",
            vec![],
            vec![("x", ty_named("Int"))],
            Some(ty_named("Int")),
            vec![ret(Some(call("leaf", vec![var("x")])))],
        ),
        vec![marker("NoGC")],
    );
    lower_user(file(vec![leaf, recursive, fun("main", vec![])]))
        .expect("resolved NoGC call graph is valid");
}

#[test]
fn generic_no_gc_records_and_propagates_gc_free_preconditions() {
    let identity = annotate(
        fun_expr(
            "identity",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(ty_named("T")),
            var("value"),
        ),
        vec![marker("NoGC")],
    );
    let forward = fun_expr(
        "forward",
        vec!["U"],
        vec![("value", ty_named("U"))],
        Some(ty_named("U")),
        typed_call("identity", vec![ty_named("U")], vec![var("value")]),
    );
    let module = lower_user(file(vec![
        identity,
        forward,
        fun(
            "main",
            vec![stmt(typed_call(
                "forward",
                vec![ty_named("Int")],
                vec![int_lit(1)],
            ))],
        ),
    ]))
    .expect("a concrete GC-free instantiation satisfies the propagated condition");

    for name in ["identity", "forward"] {
        let generic = module
            .generic_functions
            .iter()
            .map(|(_, generic)| generic)
            .find(|generic| module.functions[generic.function].name == name)
            .unwrap_or_else(|| panic!("missing generic function `{name}`"));
        assert_eq!(generic.no_gc_type_params.len(), 1);
        assert_eq!(generic.no_gc_type_params[0].into_raw(), 0);
    }
    let dump = hir::dump(&module);
    assert!(
        dump.contains("fun identity<T>(value: T0): T0 <no-gc cdecl> <requires-gc-free T>"),
        "{dump}"
    );
    assert!(
        dump.contains("fun forward<U>(value: T0): T0 <requires-gc-free U>"),
        "{dump}"
    );
}

#[test]
fn generic_no_gc_checks_concrete_arguments_but_not_phantom_parameters() {
    let identity = annotate(
        fun_expr(
            "identity",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(ty_named("T")),
            var("value"),
        ),
        vec![marker("NoGC")],
    );
    let errors = messages(vec![
        identity,
        fun(
            "main",
            vec![stmt(typed_call(
                "identity",
                vec![ty_named("String")],
                vec![str_lit("managed")],
            ))],
        ),
    ]);
    assert!(errors.iter().any(|message| {
        message == "generic function `identity` requires type argument String for `T` to be GC-free"
    }));

    let phantom = annotate(
        fun_expr(
            "phantom",
            vec!["T"],
            vec![],
            Some(ty_named("Int")),
            int_lit(1),
        ),
        vec![marker("NoGC")],
    );
    let module = lower_user(file(vec![
        phantom,
        fun(
            "main",
            vec![stmt(typed_call(
                "phantom",
                vec![ty_named("String")],
                vec![],
            ))],
        ),
    ]))
    .expect("an unused generic parameter does not affect a NoGC representation");
    let phantom = module
        .generic_functions
        .iter()
        .map(|(_, generic)| generic)
        .find(|generic| module.functions[generic.function].name == "phantom")
        .expect("phantom generic exists");
    assert!(phantom.no_gc_type_params.is_empty());
}

#[test]
fn generic_no_gc_rejects_an_impossible_ref_bound_precondition() {
    let identity = with_kind(
        annotate(
            fun_expr(
                "identity",
                vec!["T"],
                vec![("value", ty_named("T"))],
                Some(ty_named("T")),
                var("value"),
            ),
            vec![marker("NoGC")],
        ),
        ast::TypeParamKindBound::Ref,
    );
    let errors = messages(vec![identity, fun("main", vec![])]);
    assert!(errors.iter().any(|message| {
        message
            == "generic function `identity` cannot require ref-bound type parameter `T` to be GC-free"
    }));
}

#[test]
fn generic_no_gc_tracks_nested_aggregate_representations() {
    let inner = generic_struct_decl("Inner", vec!["T"], vec![("value", ty_named("T"))]);
    let outer = generic_struct_decl(
        "Outer",
        vec!["T"],
        vec![("inner", ty_generic("Inner", vec![ty_named("T")]))],
    );
    let keep = annotate(
        fun_expr(
            "keep",
            vec!["T"],
            vec![("value", ty_generic("Outer", vec![ty_named("T")]))],
            Some(ty_generic("Outer", vec![ty_named("T")])),
            var("value"),
        ),
        vec![marker("NoGC")],
    );
    lower_user(file(vec![
        inner.clone(),
        outer.clone(),
        keep.clone(),
        fun(
            "main",
            vec![stmt(typed_call(
                "keep",
                vec![ty_named("Int")],
                vec![struct_init(
                    "Outer",
                    vec![struct_init("Inner", vec![int_lit(1)])],
                )],
            ))],
        ),
    ]))
    .expect("nested aggregates preserve a concrete GC-free argument");

    let errors = messages(vec![
        inner,
        outer,
        keep,
        fun(
            "main",
            vec![stmt(typed_call(
                "keep",
                vec![ty_named("String")],
                vec![struct_init(
                    "Outer",
                    vec![struct_init("Inner", vec![str_lit("managed")])],
                )],
            ))],
        ),
    ]);
    assert!(errors.iter().any(|message| {
        message == "generic function `keep` requires type argument String for `T` to be GC-free"
    }));
}

#[test]
fn generic_no_gc_checks_value_type_receiver_instantiations() {
    let getter = annotate_method(
        method_expr("get", vec![], Some(ty_named("T")), var("value")),
        vec![marker("NoGC")],
    );
    let cell = generic_struct_decl_full(
        "Cell",
        vec!["T"],
        vec![("value", ty_named("T"))],
        vec![],
        vec![getter],
    );
    lower_user(file(vec![
        cell.clone(),
        fun(
            "main",
            vec![stmt(method_call(
                struct_init("Cell", vec![int_lit(1)]),
                "get",
                vec![],
            ))],
        ),
    ]))
    .expect("a GC-free value receiver satisfies its NoGC method condition");

    let errors = messages(vec![
        cell,
        fun(
            "main",
            vec![stmt(method_call(
                struct_init("Cell", vec![str_lit("managed")]),
                "get",
                vec![],
            ))],
        ),
    ]);
    assert!(errors.iter().any(|message| {
        message == "generic function `Cell.get` requires type argument String for `T` to be GC-free"
    }));
}

#[test]
fn no_gc_unsafe_functions_can_use_stack_addresses_and_pointer_intrinsics() {
    let function = annotate(
        fun_sig(
            "rewrite",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            vec![
                val("pointer", call("addressOf", vec![var("value")])),
                stmt(method_call(var("pointer"), "store", vec![int_lit(12)])),
                ret(Some(method_call(var("pointer"), "load", vec![]))),
            ],
        ),
        vec![marker("NoGC"), marker("Unsafe")],
    );
    lower_user(file(vec![function, fun("main", vec![])]))
        .expect("stack addressing and raw pointer operations remain GC-free");
}

#[test]
fn no_gc_rejects_managed_signatures_calls_and_implicit_exception_ops() {
    let string_param = annotate(
        fun_sig(
            "stringParam",
            vec![],
            vec![("value", ty_named("String"))],
            None,
            vec![],
        ),
        vec![marker("NoGC")],
    );
    let managed = fun("managed", vec![]);
    let calls_managed = annotate(
        fun("callsManaged", vec![stmt(call("managed", vec![]))]),
        vec![marker("NoGC")],
    );
    let divides = annotate(
        fun_expr(
            "divides",
            vec![],
            vec![],
            Some(ty_named("Int")),
            Expr::Binary {
                op: ast::BinOp::Div,
                lhs: Box::new(int_lit(4)),
                rhs: Box::new(int_lit(2)),
                span: sp(),
            },
        ),
        vec![marker("NoGC")],
    );
    let errors = messages(vec![
        string_param,
        managed,
        calls_managed,
        divides,
        fun("main", vec![]),
    ]);
    assert!(
        errors
            .iter()
            .any(|message| message.contains("non-GC-free parameter `value`"))
    );
    assert!(
        errors
            .iter()
            .any(|message| message.contains("may not call managed function `managed`"))
    );
    assert!(
        errors
            .iter()
            .any(|message| message.contains("integer division is not allowed"))
    );
}

#[test]
fn no_gc_rejects_reference_receiver_methods() {
    let method = annotate_method(method("work", vec![], None, vec![]), vec![marker("NoGC")]);
    let class = class_decl(
        ast::ClassModifier::Final,
        "Worker",
        vec![],
        None,
        vec![],
        vec![method],
    );
    let errors = messages(vec![class, fun("main", vec![])]);
    assert!(
        errors
            .iter()
            .any(|message| message.contains("parameter `this` of type Worker"))
    );
}

#[test]
fn interior_mutable_values_require_unsafe_use_and_unsafe_signatures() {
    let mut cell = struct_decl("Cell", vec![("value", ty_named("Int"))]);
    let Decl::Struct(cell_decl) = &mut cell else {
        unreachable!()
    };
    cell_decl.annotations = vec![marker("InteriorMutable")];

    let errors = messages(vec![
        cell.clone(),
        fun(
            "main",
            vec![val("cell", struct_init("Cell", vec![int_lit(1)]))],
        ),
    ]);
    assert!(
        errors
            .iter()
            .any(|message| message.contains("requires an unsafe context"))
    );

    lower_user(file(vec![
        cell.clone(),
        fun(
            "main",
            vec![safety_block(
                ast::SafetyMode::Unsafe,
                vec![val("cell", struct_init("Cell", vec![int_lit(1)]))],
            )],
        ),
    ]))
    .expect("unsafe block authorizes interior-mutable use");

    let safe_signature = annotate(
        fun_sig(
            "exposes",
            vec![],
            vec![("cell", ty_named("Cell"))],
            None,
            vec![],
        ),
        vec![marker("Safe")],
    );
    let errors = messages(vec![cell, safe_signature, fun("main", vec![])]);
    assert!(errors.iter().any(|message| {
        message.contains("safe function `exposes` exposes `@InteriorMutable` parameter")
    }));
}

#[test]
fn interior_mutable_classification_substitutes_nested_generic_fields() {
    let mut cell = struct_decl("Cell", vec![("value", ty_named("Int"))]);
    let Decl::Struct(cell_decl) = &mut cell else {
        unreachable!()
    };
    cell_decl.annotations = vec![marker("InteriorMutable")];
    let wrapper = generic_struct_decl("Wrapper", vec!["T"], vec![("value", ty_named("T"))]);
    let outer = generic_struct_decl(
        "Outer",
        vec!["T"],
        vec![("wrapper", ty_generic("Wrapper", vec![ty_named("T")]))],
    );
    let nested = ty_generic("Outer", vec![ty_named("Cell")]);
    let errors = messages(vec![
        cell.clone(),
        wrapper.clone(),
        outer.clone(),
        fun_sig(
            "exposes",
            vec![],
            vec![("nested", nested.clone())],
            None,
            vec![],
        ),
        fun("main", vec![]),
    ]);
    assert!(errors.iter().any(|message| {
        message.contains("safe function `exposes` exposes `@InteriorMutable` parameter `nested`")
    }));

    let unsafe_function = annotate(
        fun_sig("accepts", vec![], vec![("nested", nested)], None, vec![]),
        vec![marker("Unsafe")],
    );
    lower_user(file(vec![
        cell,
        wrapper,
        outer,
        unsafe_function,
        fun("main", vec![]),
    ]))
    .expect("an unsafe signature may expose recursively interior-mutable state");
}

#[test]
fn extern_functions_have_typed_identity_and_abi_specific_effects() {
    let c = extern_fun(
        "nativeAdd",
        vec![("left", ty_named("Int")), ("right", ty_named("Int"))],
        Some(ty_named("Int")),
        extern_annotation("numbers", "native_add", "c"),
    );
    let scoop = extern_fun(
        "nativeWrite",
        vec![("message", ty_named("String"))],
        None,
        extern_annotation("", "scoop_rt_write", "scoop"),
    );
    let module = lower_user(file(vec![c, scoop, fun("main", vec![])]))
        .expect("both extern ABI categories must lower");
    assert_eq!(module.extern_functions.len(), 3);
    let (_, c) = module.extern_functions.iter().nth(1).unwrap();
    assert_eq!(c.abi, hir::ExternAbi::C);
    assert_eq!(c.safety, hir::Safety::Unsafe);
    assert_eq!(c.gc_effect, hir::GcEffect::NoGc);
    let (_, scoop) = module.extern_functions.iter().nth(2).unwrap();
    assert_eq!(scoop.abi, hir::ExternAbi::Scoop);
    assert_eq!(scoop.safety, hir::Safety::Safe);
    assert_eq!(scoop.gc_effect, hir::GcEffect::Managed);
    let dump = hir::dump(&module);
    assert!(dump.contains("<extern1 abi=c symbol=native_add lib=numbers>"));
    assert!(dump.contains("<extern2 abi=scoop symbol=scoop_rt_write>"));
}

#[test]
fn extern_functions_reject_invalid_declarations_and_boundary_types() {
    let c_string = extern_fun(
        "cString",
        vec![("value", ty_named("String"))],
        None,
        extern_annotation("", "c_string", "c"),
    );
    let mut generic = extern_fun(
        "generic",
        vec![("value", ty_named("T"))],
        None,
        extern_annotation("", "generic", "c"),
    );
    let Decl::Function(generic_function) = &mut generic else {
        unreachable!()
    };
    generic_function.type_params = vec![type_param("T")];
    let mut suspend = extern_fun(
        "waitNative",
        vec![],
        None,
        extern_annotation("", "wait_native", "scoop"),
    );
    let Decl::Function(suspend_function) = &mut suspend else {
        unreachable!()
    };
    suspend_function.is_suspend = true;
    let errors = messages(vec![c_string, generic, suspend, fun("main", vec![])]);
    assert!(
        errors
            .iter()
            .any(|message| message.contains("ref type `String` is managed"))
    );
    assert!(
        errors
            .iter()
            .any(|message| message.contains("must not be generic"))
    );
    assert!(
        errors
            .iter()
            .any(|message| message.contains("cannot be used on a suspend"))
    );
}

#[test]
fn duplicate_native_symbols_must_have_one_consistent_contract() {
    let first = extern_fun(
        "first",
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        extern_annotation("one", "same_symbol", "c"),
    );
    let second = extern_fun(
        "second",
        vec![("value", ty_named("UInt"))],
        Some(ty_named("UInt")),
        extern_annotation("two", "same_symbol", "c"),
    );
    let errors = messages(vec![first, second, fun("main", vec![])]);
    assert!(
        errors
            .iter()
            .any(|message| message.contains("extern symbol `same_symbol` conflicts"))
    );

    let managed = extern_fun(
        "managedAlias",
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        extern_annotation("runtime", "same_scoop_symbol", "scoop"),
    );
    let mut no_gc = extern_fun(
        "noGcAlias",
        vec![("value", ty_named("Int"))],
        Some(ty_named("Int")),
        extern_annotation("runtime", "same_scoop_symbol", "scoop"),
    );
    let Decl::Function(no_gc_function) = &mut no_gc else {
        unreachable!()
    };
    no_gc_function.annotations.push(marker("NoGC"));
    let errors = messages(vec![managed, no_gc, fun("main", vec![])]);
    assert!(
        errors
            .iter()
            .any(|message| message.contains("extern symbol `same_scoop_symbol` conflicts"))
    );
}
