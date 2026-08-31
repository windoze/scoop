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
        vec![string_annotation("Intrinsic", "rt_write")],
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
