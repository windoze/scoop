use super::*;

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
fn generic_no_gc_method_tracks_owner_and_method_preconditions() {
    let mut keep = annotate_method(
        method_expr(
            "keep",
            vec![("other", ty_named("U"))],
            Some(ty_named("U")),
            var("other"),
        ),
        vec![marker("NoGC")],
    );
    keep.type_params = vec![type_param("U")];
    let cell = generic_struct_decl_full(
        "Cell",
        vec!["T"],
        vec![("value", ty_named("T"))],
        vec![],
        vec![keep],
    );

    let module = lower_user(file(vec![
        cell.clone(),
        fun(
            "main",
            vec![stmt(method_call(
                struct_init("Cell", vec![int_lit(1)]),
                "keep",
                vec![int_lit(2)],
            ))],
        ),
    ]))
    .expect("both exact type-argument groups are GC-free");
    let method = module
        .generic_methods
        .iter()
        .map(|(_, method)| method)
        .find(|method| module.functions[method.function].name == "Cell.keep")
        .expect("Cell.keep generic method entity");
    assert_eq!(
        method
            .no_gc_type_params
            .iter()
            .map(|parameter| parameter.into_raw())
            .collect::<Vec<_>>(),
        [0, 1]
    );

    let owner_errors = messages(vec![
        cell.clone(),
        fun(
            "main",
            vec![stmt(method_call(
                struct_init("Cell", vec![str_lit("managed owner")]),
                "keep",
                vec![int_lit(2)],
            ))],
        ),
    ]);
    assert!(owner_errors.iter().any(|message| {
        message
            == "generic function `Cell.keep` requires type argument String for `T` to be GC-free"
    }));

    let method_errors = messages(vec![
        cell.clone(),
        fun(
            "main",
            vec![stmt(method_call(
                struct_init("Cell", vec![int_lit(1)]),
                "keep",
                vec![str_lit("managed method argument")],
            ))],
        ),
    ]);
    assert!(method_errors.iter().any(|message| {
        message
            == "generic function `Cell.keep` requires type argument String for `U` to be GC-free"
    }));

    let reference = ast::Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: Some(Box::new(struct_init("Cell", vec![int_lit(1)]))),
        name: ident("keep"),
        span: sp(),
    };
    let reference_errors = messages(vec![
        cell,
        fun(
            "main",
            vec![val_ty(
                "keepString",
                Some(ty_function(
                    false,
                    vec![ty_named("String")],
                    ty_named("String"),
                )),
                reference,
            )],
        ),
    ]);
    assert!(reference_errors.iter().any(|message| {
        message
            == "generic function `Cell.keep` requires type argument String for `U` to be GC-free"
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
