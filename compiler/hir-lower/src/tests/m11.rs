use scoop_hir as hir;

use super::*;

#[test]
fn function_types_are_canonical_and_keep_suspend_identity() {
    let ordinary = ty_function(false, vec![ty_named("Int")], ty_named("String"));
    let suspend = ty_function(true, vec![ty_named("Int")], ty_named("String"));
    let module = lower_user(file(vec![
        fun_sig(
            "use",
            vec![],
            vec![
                ("first", ordinary.clone()),
                ("second", ordinary),
                ("task", suspend),
            ],
            None,
            vec![],
        ),
        fun("main", vec![]),
    ]))
    .expect("function types should lower");

    let use_ = module
        .top_level
        .iter()
        .map(|id| &module.functions[*id])
        .find(|function| function.name == "use")
        .expect("use function");
    assert_eq!(use_.params[0].ty, use_.params[1].ty);
    assert_ne!(use_.params[0].ty, use_.params[2].ty);
    assert_eq!(module.function_types.len(), 2);
    assert_eq!(
        hir::type_name(&module, use_.params[0].ty),
        "(Int) -> String"
    );
    assert_eq!(
        hir::type_name(&module, use_.params[2].ty),
        "suspend (Int) -> String"
    );
}

#[test]
fn function_types_substitute_type_parameters_recursively() {
    let generic = ty_function(false, vec![ty_named("T")], ty_named("T"));
    let module = lower_user(file(vec![
        fun_sig("consume", vec!["T"], vec![("op", generic)], None, vec![]),
        fun("main", vec![]),
    ]))
    .expect("generic function type should lower");

    let consume = module
        .top_level
        .iter()
        .map(|id| &module.functions[*id])
        .find(|function| function.name == "consume")
        .expect("consume function");
    let hir::Type::Function(id) = module.types[consume.params[0].ty] else {
        panic!("expected function type");
    };
    let signature = &module.function_types[id];
    assert!(matches!(
        module.types[signature.parameter_types[0]],
        hir::Type::Param(_)
    ));
    assert_eq!(signature.parameter_types[0], signature.return_type);
}

#[test]
fn function_types_are_reference_types_for_gc_constraints() {
    let mut lowerer = Lowerer::new();
    let ty = lowerer.intern_function_type(false, vec![lowerer.int], lowerer.string);
    assert!(lowerer.is_ref_ty(ty));
    assert!(!lowerer.is_value_ty(ty));
}

fn lambda(parameters: Option<Vec<ast::LambdaParam>>, tail: ast::Expr) -> ast::Expr {
    ast::Expr::Lambda {
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters,
        body: block(vec![stmt(tail)]),
        span: sp(),
    }
}

fn anonymous(
    params: Vec<(&str, ast::TypeRef)>,
    return_ty: Option<ast::TypeRef>,
    statements: Vec<ast::Statement>,
) -> ast::Expr {
    ast::Expr::AnonymousFunction {
        id: ast::AnonymousFunctionId(0),
        is_suspend: false,
        params: params
            .into_iter()
            .map(|(name, ty)| ast::Param {
                name: ident(name),
                ty,
                span: sp(),
            })
            .collect(),
        return_ty,
        body: block(statements),
        span: sp(),
    }
}

#[test]
fn expected_lambda_builds_a_typed_invoke_function_and_callable_call() {
    let signature = ty_function(false, vec![ty_named("Int")], ty_named("Int"));
    let operation = lambda(
        Some(vec![ast::LambdaParam {
            target: pat_bind("value"),
            ty: None,
            span: sp(),
        }]),
        binary(ast::BinOp::Add, var("value"), int_lit(1)),
    );
    let module = lower_user(file(vec![fun(
        "main",
        vec![
            val_ty("operation", Some(signature), operation),
            stmt(call("operation", vec![int_lit(41)])),
        ],
    )]))
    .expect("expected type should type the lambda parameter");

    assert_eq!(module.lambdas.len(), 1);
    let (_, lambda) = module.lambdas.iter().next().expect("lambda entity");
    assert!(lambda.captures.is_empty());
    let invoke = &module.functions[lambda.function];
    assert_eq!(
        invoke.params.len(),
        2,
        "closure receiver plus source parameter"
    );
    assert_eq!(invoke.params[0].name, "$closure");
    let main = &module.functions[module.entry];
    let hir::FunctionKind::User(body) = &main.kind else {
        panic!("main body");
    };
    assert!(matches!(
        body.statements[1].kind,
        hir::StatementKind::Expr(hir::Expr {
            kind: hir::ExprKind::CallableCall { .. },
            ..
        })
    ));
}

#[test]
fn top_level_reference_is_a_distinct_typed_entity() {
    let signature = ty_function(false, vec![ty_named("Int")], ty_named("Int"));
    let reference = ast::Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: None,
        name: ident("increment"),
        span: sp(),
    };
    let module = lower_user(file(vec![
        fun_expr(
            "increment",
            vec![],
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            var("value"),
        ),
        fun(
            "main",
            vec![val_ty("operation", Some(signature), reference)],
        ),
    ]))
    .expect("top-level reference should lower");
    assert_eq!(module.callable_references.len(), 1);
    let (_, reference) = module
        .callable_references
        .iter()
        .next()
        .expect("reference entity");
    assert!(reference.captures.is_empty());
    let target = module.callable_function(reference.target);
    assert_eq!(module.functions[target].name, "increment");
}

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

#[test]
fn anonymous_function_infers_return_and_owns_local_return() {
    let operation = anonymous(
        vec![("value", ty_named("Int"))],
        None,
        vec![ret(Some(binary(
            ast::BinOp::Add,
            var("value"),
            var("base"),
        )))],
    );
    let module = lower_user(file(vec![fun(
        "main",
        vec![
            val("base", int_lit(1)),
            val("operation", operation),
            stmt(call("operation", vec![int_lit(41)])),
        ],
    )]))
    .expect("anonymous function should infer its return type");

    let (_, anonymous) = module
        .anonymous_functions
        .iter()
        .next()
        .expect("anonymous function entity");
    assert_eq!(anonymous.captures.len(), 1);
    let signature = &module.function_types[anonymous.function_type];
    assert_eq!(signature.parameter_types, vec![module.int]);
    assert_eq!(signature.return_type, module.int);
    let invoke = &module.functions[anonymous.function];
    let hir::FunctionKind::User(body) = &invoke.kind else {
        panic!("anonymous invoke body");
    };
    assert!(matches!(
        body.statements.last().map(|statement| &statement.kind),
        Some(hir::StatementKind::Return { value: Some(_) })
    ));
}
