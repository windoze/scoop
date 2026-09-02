use super::*;

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
