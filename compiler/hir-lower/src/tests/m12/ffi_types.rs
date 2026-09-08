use super::*;

fn integer_with_suffix(magnitude: u64, suffix: ast::IntegerSuffix) -> Expr {
    Expr::IntLiteral(ast::IntegerLiteralSyntax {
        magnitude,
        radix: ast::IntegerRadix::Decimal,
        suffix,
        span: sp(),
    })
}

fn integer_infix(lhs: Expr, name: &str, rhs: Expr) -> Expr {
    Expr::InfixCall {
        lhs: Box::new(lhs),
        target: ast::InfixTarget::Named(ident(name)),
        rhs: Box::new(rhs),
        span: sp(),
    }
}

fn const_property(name: &str, ty: TypeRef, expression: Expr) -> Decl {
    Decl::Global(ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty,
        body: ast::PropertyBodySyntax::Const(Box::new(expression)),
        span: sp(),
    })
}

fn raw_storage_without_initializer(name: &str, ty: TypeRef) -> Decl {
    Decl::Global(ast::PropertyDecl {
        annotations: vec![marker("Global")],
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: true,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty,
        body: ast::PropertyBodySyntax::ExternStorage,
        span: sp(),
    })
}

fn raw_storage_with_initializer(name: &str, ty: TypeRef, expression: Expr) -> Decl {
    Decl::Global(ast::PropertyDecl {
        annotations: vec![marker("Global")],
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: true,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty,
        body: ast::PropertyBodySyntax::Initializer {
            expression: Box::new(expression),
            accessors: ast::AccessorSyntax::default(),
        },
        span: sp(),
    })
}

#[test]
fn pointer_core_normalizes_construction_memory_ops_and_layout_queries() {
    let pointer_ty = ty_generic("Ptr", vec![ty_named("Int")]);
    let program = file(vec![fun(
        "main",
        vec![
            val_ty(
                "raw",
                Some(ty_named("ULong")),
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
                        Some(ty_named("ULong")),
                        typed_call("alignOf", vec![ty_named("Int")], vec![]),
                    ),
                ],
            ),
        ],
    )]);
    let module = lower_user(program).expect("typed pointer operations must lower");
    let dump = hir::dump(&module);
    for node in [
        "PtrFromNonZeroULong",
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
    assert!(
        module
            .types
            .iter()
            .any(|(_, ty)| matches!(ty, hir::Type::Ptr(pointee)
            if module.types[*pointee] == hir::Type::Integer(hir::IntegerKind::SIGNED_32)))
    );
    let top_level_intrinsics = [
        module.ffi_core.address_of,
        module.ffi_core.size_of,
        module.ffi_core.align_of,
    ];
    assert!(module.instantiations.iter().all(|(_, application)| {
        let function = module.generic_functions[application.generic].function;
        !top_level_intrinsics.contains(&function)
    }));
    let pointer_methods = [
        module.ffi_core.ptr_to_ulong,
        module.ffi_core.ptr_cast,
        module.ffi_core.ptr_load,
        module.ffi_core.ptr_load_offset,
        module.ffi_core.ptr_store,
        module.ffi_core.ptr_store_offset,
        module.ffi_core.ptr_plus,
        module.ffi_core.ptr_minus,
    ];
    assert!(
        module
            .method_applications
            .iter()
            .all(|(_, application)| { !pointer_methods.contains(&application.function) })
    );
    assert!(
        module
            .generic_method_applications
            .iter()
            .all(|(_, application)| {
                let function = module.generic_methods[application.method].function;
                !pointer_methods.contains(&function)
            })
    );
}

#[test]
fn pointer_carrier_equality_uses_the_typed_ulong_intrinsic() {
    let ulong = |value| integer_with_suffix(value, ast::IntegerSuffix::UnsignedLong);
    let pointer_type = ty_generic("Ptr", vec![ty_named("Int")]);
    let module = lower_user(file(vec![fun(
        "main",
        vec![safety_block(
            ast::SafetyMode::Unsafe,
            vec![
                val_ty(
                    "pointer",
                    Some(pointer_type.clone()),
                    typed_call("Ptr", vec![ty_named("Int")], vec![ulong(1)]),
                ),
                val_ty(
                    "expected",
                    Some(pointer_type),
                    typed_call("Ptr", vec![ty_named("Int")], vec![ulong(1)]),
                ),
                val(
                    "sameAddress",
                    binary(
                        BinOp::Eq,
                        method_call(var("pointer"), "toULong", Vec::new()),
                        method_call(var("expected"), "toULong", Vec::new()),
                    ),
                ),
                val("sameULong", binary(BinOp::Eq, ulong(1), ulong(1))),
            ],
        )],
    )]))
    .expect("pointer carrier equality must normalize before MIR");
    let hir::FunctionKind::User(body) = &module.functions[module.entry].kind else {
        panic!("entry function must have a user body")
    };
    for name in ["sameAddress", "sameULong"] {
        assert!(matches!(
            &local_init(body, name).kind,
            hir::ExprKind::IntegerOperation {
                operation: hir::IntegerOperation::NoGc {
                    kind: hir::IntegerKind::UNSIGNED_64,
                    operation: hir::NoGcIntegerOperation::Equals,
                    ..
                },
                ..
            }
        ));
    }
}

#[test]
fn pointer_construction_maps_named_raw_and_preserves_raw_contract() {
    let ulong = |value| integer_with_suffix(value, ast::IntegerSuffix::UnsignedLong);
    let pointer = |raw| {
        typed_source_call(
            "Ptr",
            vec![ty_named("Int")],
            vec![named_argument("raw", raw)],
        )
    };
    let module = lower_user(file(vec![fun(
        "main",
        vec![safety_block(
            ast::SafetyMode::Unsafe,
            vec![val("pointer", pointer(ulong(1)))],
        )],
    )]))
    .expect("the named raw argument must bind to the Ptr constructor parameter");
    let hir::FunctionKind::User(body) = &module.functions[module.entry].kind else {
        panic!("entry function must have a user body")
    };
    assert!(matches!(
        &local_init(body, "pointer").kind,
        hir::ExprKind::PtrFromNonZeroULong(raw)
            if module.types[raw.ty]
                == hir::Type::Integer(hir::IntegerKind::UNSIGNED_64)
    ));

    let errors = messages(vec![fun(
        "main",
        vec![
            val_ty("raw", Some(ty_named("Int")), int_lit(1)),
            safety_block(
                ast::SafetyMode::Unsafe,
                vec![stmt(pointer(var("raw"))), stmt(pointer(ulong(0)))],
            ),
        ],
    )]);
    assert!(
        errors
            .iter()
            .any(|message| message == "`Ptr` raw address must be ULong, found Int"),
        "{errors:#?}"
    );
    assert!(
        errors
            .iter()
            .any(|message| message == "`Ptr` raw address must be nonzero"),
        "{errors:#?}"
    );
}

#[test]
fn pointer_construction_rejects_invalid_value_argument_shapes() {
    let ulong = || integer_with_suffix(1, ast::IntegerSuffix::UnsignedLong);
    let pointer = |args| typed_source_call("Ptr", vec![ty_named("Int")], args);
    let errors = messages(vec![fun(
        "main",
        vec![safety_block(
            ast::SafetyMode::Unsafe,
            vec![
                stmt(pointer(Vec::new())),
                stmt(pointer(vec![named_argument("address", ulong())])),
                stmt(pointer(vec![
                    named_argument("raw", ulong()),
                    named_argument("raw", ulong()),
                ])),
                stmt(pointer(vec![spread_argument(ulong())])),
            ],
        )],
    )]);
    assert_eq!(
        errors,
        vec![
            "required `Ptr` value parameter `raw` has no argument",
            "`Ptr` has no value parameter named `address`",
            "`Ptr` value parameter `raw` is supplied more than once",
            "spread is not allowed for `Ptr` value parameter `raw`",
        ]
    );
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
    assert_eq!(errors, vec!["`Ptr` pointee must be GC-free, found TextBox"]);
}

#[test]
fn pointer_construction_rejects_every_folded_constant_zero_address() {
    let ulong = |value| integer_with_suffix(value, ast::IntegerSuffix::UnsignedLong);
    let uint = |value| integer_with_suffix(value, ast::IntegerSuffix::Unsigned);
    let long = |value| integer_with_suffix(value, ast::IntegerSuffix::Long);
    let construct = |raw| typed_call("Ptr", vec![ty_named("Int")], vec![raw]);
    let errors = messages(vec![
        const_property("zero", ty_named("ULong"), ulong(0)),
        fun(
            "main",
            vec![safety_block(
                ast::SafetyMode::Unsafe,
                vec![
                    stmt(construct(ulong(0))),
                    stmt(construct(var("zero"))),
                    stmt(construct(method_call(uint(0), "toUInt64", Vec::new()))),
                    stmt(construct(integer_infix(ulong(1), "and", ulong(0)))),
                    stmt(construct(integer_infix(ulong(0), "shl", long(1)))),
                ],
            )],
        ),
    ]);
    assert_eq!(
        errors
            .iter()
            .filter(|message| message.as_str() == "`Ptr` raw address must be nonzero")
            .count(),
        5,
        "{errors:#?}"
    );
}

#[test]
fn pointer_representation_has_no_source_fields_or_destructuring_shape() {
    let pointer = || {
        typed_call(
            "Ptr",
            vec![ty_named("Int")],
            vec![integer_with_suffix(1, ast::IntegerSuffix::UnsignedLong)],
        )
    };
    let field_errors = messages(vec![fun(
        "main",
        vec![safety_block(
            ast::SafetyMode::Unsafe,
            vec![
                val_ty(
                    "pointer",
                    Some(ty_generic("Ptr", vec![ty_named("Int")])),
                    pointer(),
                ),
                val("raw", field(var("pointer"), "raw")),
            ],
        )],
    )]);
    assert!(
        field_errors
            .iter()
            .any(|message| { message.contains("Ptr<Int>") && message.contains("has no fields") }),
        "{field_errors:#?}"
    );

    let destructuring_errors = messages(vec![fun(
        "main",
        vec![safety_block(
            ast::SafetyMode::Unsafe,
            vec![
                val_ty(
                    "pointer",
                    Some(ty_generic("Ptr", vec![ty_named("Int")])),
                    pointer(),
                ),
                val_pat(
                    false,
                    pat_named(&["Ptr"], vec![("raw", None)], None),
                    None,
                    var("pointer"),
                ),
            ],
        )],
    )]);
    assert!(
        destructuring_errors.iter().any(|message| {
            message == "intrinsic pointer type `Ptr<Int>` cannot be destructured"
        }),
        "{destructuring_errors:#?}"
    );
}

#[test]
fn bare_pointer_storage_has_no_all_zero_initial_image() {
    let signature = ty_function(false, vec![ty_named("Int")], ty_named("Unit"));
    let errors = messages(vec![
        raw_storage_without_initializer("pointer", ty_generic("Ptr", vec![ty_named("Int")])),
        raw_storage_without_initializer("callback", ty_generic("FunPtr", vec![signature])),
        fun("main", Vec::new()),
    ]);
    assert!(errors.iter().any(|message| {
        message == "raw storage of type Ptr<Int> has no valid all-zero initial image"
    }));
    assert!(errors.iter().any(|message| {
        message == "raw storage of type FunPtr<(Int) -> Unit> has no valid all-zero initial image"
    }));
}

#[test]
fn nullable_pointer_storage_accepts_none_as_an_encoded_constant_image() {
    let signature = ty_function(false, vec![ty_named("Int")], ty_named("Unit"));
    let module = lower_user(file(vec![
        raw_storage_with_initializer(
            "pointer",
            ty_nullable(ty_generic("Ptr", vec![ty_named("Int")])),
            none(),
        ),
        raw_storage_with_initializer(
            "callback",
            ty_nullable(ty_generic("FunPtr", vec![signature])),
            none(),
        ),
        fun("main", Vec::new()),
    ]))
    .expect("nullable pointer None values must be valid raw-storage images");

    for name in ["pointer", "callback"] {
        let global = module
            .globals
            .iter()
            .find_map(|(_, global)| {
                (module.properties[global.property].name == name).then_some(global)
            })
            .unwrap_or_else(|| panic!("missing global {name}"));
        assert!(matches!(
            global.storage,
            hir::GlobalStorage::Local {
                initializer: hir::HirConstantImage::EnumUnit { .. },
                ..
            }
        ));
    }
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
fn fun_ptr_signature_is_an_indirection_boundary_for_recursive_c_layouts() {
    let node = annotate_struct(
        struct_decl(
            "Node",
            vec![(
                "handler",
                ty_generic(
                    "FunPtr",
                    vec![ty_function(false, vec![ty_named("Node")], ty_named("Unit"))],
                ),
            )],
        ),
        vec![c_layout(0, 0)],
    );
    lower_user(file(vec![node, fun("main", vec![])]))
        .expect("a recursive callback signature is not a by-value C layout cycle");
}

#[test]
fn c_pointer_pointees_must_be_nonzero_sized_portable_objects() {
    let plain = struct_decl("Plain", vec![("value", ty_named("Int"))]);
    let empty = struct_decl("Empty", Vec::new());
    let plain_pointer = extern_fun(
        "consumePlain",
        vec![("value", ty_generic("Ptr", vec![ty_named("Plain")]))],
        None,
        extern_annotation("", "consume_plain", "c"),
    );
    let empty_pointer = extern_fun(
        "consumeEmpty",
        vec![("value", ty_generic("Ptr", vec![ty_named("Empty")]))],
        None,
        extern_annotation("", "consume_empty", "c"),
    );
    let errors = messages(vec![
        plain,
        empty,
        plain_pointer,
        empty_pointer,
        fun("main", vec![]),
    ]);
    assert!(
        errors
            .iter()
            .any(|message| { message.contains("ordinary struct `Plain` has no stable C layout") })
    );
    assert!(errors.iter().any(|message| {
        message.contains("pointer pointee `Empty` is zero-sized")
            && message.contains("only `Ptr<Unit>` maps to `void *`")
    }));

    let unit_pointer = extern_fun(
        "consumeOpaque",
        vec![("value", ty_generic("Ptr", vec![ty_named("Unit")]))],
        None,
        extern_annotation("", "consume_opaque", "c"),
    );
    lower_user(file(vec![unit_pointer, fun("main", vec![])]))
        .expect("Ptr<Unit> is the canonical C void pointer");
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
    let invalid_callback = annotate_struct(
        struct_decl(
            "InvalidCallback",
            vec![("callback", ty_generic("FunPtr", vec![signature]))],
        ),
        vec![c_layout(0, 0)],
    );
    let errors = messages(vec![empty, invalid_callback, fun("main", vec![])]);
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
fn native_function_reference_is_distinct_from_managed_values() {
    let signature = ty_function(false, vec![ty_named("Int")], ty_named("Int"));
    let fun_ptr = ty_generic("FunPtr", vec![signature]);
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
        fun("main", vec![val_ty("callback", Some(fun_ptr), reference)]),
    ]))
    .expect("a native function address must lower");
    let dump = hir::dump(&module);
    assert!(dump.contains("FunctionAddress increment"));
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
            .any(|message| message.contains("native callbacks must be declared `@NoGC`"))
    );
}

#[test]
fn fun_ptr_has_no_source_constructor_and_requires_a_function_argument() {
    let signature = ty_function(false, vec![ty_named("Int")], ty_named("Int"));
    let errors = messages(vec![fun(
        "main",
        vec![
            stmt(typed_call("FunPtr", vec![signature.clone()], vec![])),
            stmt(typed_call(
                "FunPtr",
                vec![signature],
                vec![integer_with_suffix(1, ast::IntegerSuffix::UnsignedLong)],
            )),
            val_ty(
                "invalid",
                Some(ty_generic("FunPtr", vec![ty_named("Int")])),
                unit_lit(),
            ),
        ],
    )]);
    assert_eq!(
        errors
            .iter()
            .filter(|message| {
                message.as_str() == "intrinsic struct `FunPtr` has no source constructor"
            })
            .count(),
        2
    );
    assert!(errors.iter().any(|message| {
        message.contains("type argument")
            && message.contains("FunPtr")
            && message.contains("function")
    }));
}
