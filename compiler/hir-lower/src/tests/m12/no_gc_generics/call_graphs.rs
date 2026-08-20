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
        dump.contains("fun identity<T>(value: T0): T0 <no-gc cdecl> <no-transition> <requires-release-value T> <requires-gc-free T>"),
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
