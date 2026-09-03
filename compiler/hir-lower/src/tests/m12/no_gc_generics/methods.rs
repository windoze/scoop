use super::*;

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
