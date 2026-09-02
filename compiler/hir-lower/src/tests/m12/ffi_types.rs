use super::*;

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
