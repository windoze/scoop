use super::*;

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
