use super::*;

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
