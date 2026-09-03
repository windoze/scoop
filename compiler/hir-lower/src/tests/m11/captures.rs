use scoop_hir as hir;

use super::*;

#[test]
fn lambda_capture_uses_global_binding_identity_and_creation_source() {
    let operation_ty = ty_function(false, vec![ty_named("Int")], ty_named("Int"));
    let operation = lambda(
        Some(vec![ast::LambdaParam {
            target: pat_bind("value"),
            ty: Some(ty_named("Int")),
            span: sp(),
        }]),
        binary(ast::BinOp::Add, var("value"), var("base")),
    );
    let module = lower_user(file(vec![
        fun_expr(
            "make",
            vec![],
            vec![("base", ty_named("Int"))],
            Some(operation_ty),
            operation,
        ),
        fun("main", vec![]),
    ]))
    .expect("immutable parameter capture should lower");

    let make_id = module
        .top_level
        .iter()
        .copied()
        .find(|id| module.functions[*id].name == "make")
        .expect("make function");
    let source_local = module.functions[make_id].params[0].local;
    let source_binding = match &module.functions[make_id].kind {
        hir::FunctionKind::User(body) => body.locals[source_local].binding,
        _ => panic!("make body"),
    };
    let (_, lambda) = module.lambdas.iter().next().expect("lambda entity");
    let [capture] = lambda.captures.as_slice() else {
        panic!("one capture");
    };
    assert_eq!(capture.binding, source_binding);
    assert_eq!(capture.name, "base");
    assert!(matches!(
        capture.source.kind,
        hir::ExprKind::Local(local) if local == source_local
    ));
}

#[test]
fn nested_lambda_propagates_transitive_capture() {
    let inner = lambda(None, var("base"));
    let outer = lambda(None, inner);
    let nested_ty = ty_function(false, vec![], ty_function(false, vec![], ty_named("Int")));
    let module = lower_user(file(vec![
        fun_expr(
            "make",
            vec![],
            vec![("base", ty_named("Int"))],
            Some(nested_ty),
            outer,
        ),
        fun("main", vec![]),
    ]))
    .expect("nested capture should lower");

    assert_eq!(module.lambdas.len(), 2);
    let mut captures: Vec<_> = module
        .lambdas
        .iter()
        .map(|(_, lambda)| &lambda.captures[0])
        .collect();
    captures.sort_by_key(|capture| match capture.source.kind {
        hir::ExprKind::Local(_) => 0,
        hir::ExprKind::Capture(_) => 1,
        _ => 2,
    });
    assert_eq!(captures[0].binding, captures[1].binding);
    assert!(matches!(captures[0].source.kind, hir::ExprKind::Local(_)));
    assert!(matches!(
        captures[1].source.kind,
        hir::ExprKind::Capture(binding) if binding == captures[0].binding
    ));
}

#[test]
fn mutable_local_capture_is_rejected_at_the_use() {
    let errors = lower_user(file(vec![fun(
        "main",
        vec![
            var_("value", int_lit(0)),
            val("operation", lambda(None, var("value"))),
        ],
    )]))
    .expect_err("mutable capture must fail");
    assert_eq!(errors.len(), 1);
    assert_eq!(
        errors[0].message,
        "cannot capture mutable local `value`; bind its current value to a `val` snapshot or capture explicit reference state"
    );
}
