use scoop_hir as hir;

use super::*;

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
    let main = &module.functions[module.entry()];
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
fn unit_returning_lambda_discards_its_tail_value_explicitly() {
    let signature = ty_function(false, vec![ty_named("Int")], ty_named("Unit"));
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
        vec![val_ty("operation", Some(signature), operation)],
    )]))
    .expect("a Unit-returning lambda discards a non-Unit tail value");

    let (_, lambda) = module.lambdas.iter().next().expect("lambda entity");
    let invoke = &module.functions[lambda.function];
    assert_eq!(invoke.return_ty, module.unit);
    let hir::FunctionKind::User(body) = &invoke.kind else {
        panic!("lambda body");
    };
    assert!(body.statements.iter().any(|statement| matches!(
        statement.kind,
        hir::StatementKind::Expr(hir::Expr {
            kind: hir::ExprKind::IntegerOperation { .. },
            ..
        })
    )));
    assert!(
        body.statements
            .iter()
            .any(|statement| matches!(statement.kind, hir::StatementKind::Return { value: None }))
    );
    assert!(!body.statements.iter().any(|statement| matches!(
        statement.kind,
        hir::StatementKind::Return { value: Some(_) }
    )));
}

#[test]
fn nested_lambdas_reserve_distinct_body_names_before_lowering() {
    let signature = ty_function(false, Vec::new(), ty_named("Int"));
    let inner = lambda(Some(Vec::new()), int_lit(42));
    let outer = ast::Expr::Lambda {
        id: ast::LambdaId(1),
        is_suspend: false,
        parameters: Some(Vec::new()),
        body: block(vec![
            val_ty("inner", Some(signature.clone()), inner),
            stmt(call("inner", Vec::new())),
        ]),
        span: sp(),
    };
    let module = lower_user(file(vec![fun(
        "main",
        vec![val_ty("outer", Some(signature), outer)],
    )]))
    .expect("nested lambdas with the same signature must have distinct bodies");

    let names: std::collections::HashSet<_> = module
        .lambdas
        .iter()
        .map(|(_, lambda)| module.functions[lambda.function].name.as_str())
        .collect();
    assert_eq!(module.lambdas.len(), 2);
    assert_eq!(names.len(), 2);
    assert!(names.contains("$lambda.0"));
    assert!(names.contains("$lambda.1"));
    assert!(module.lambdas.iter().all(|(_, lambda)| {
        lambda.definition_root == hir::LexicalDefinitionRoot::Function(module.entry())
    }));
    let paths = module
        .lambdas
        .iter()
        .map(|(_, lambda)| definition_path(&lambda.definition_path))
        .collect::<Vec<_>>();
    assert!(paths.contains(&vec![(
        scoop_identity::StructuralDefinitionSiteRole::Lambda,
        0,
    )]));
    assert!(paths.contains(&vec![
        (scoop_identity::StructuralDefinitionSiteRole::Lambda, 0),
        (scoop_identity::StructuralDefinitionSiteRole::Lambda, 0),
    ]));
}

#[test]
fn nested_anonymous_functions_reserve_distinct_body_names_before_lowering() {
    let signature = ty_function(false, Vec::new(), ty_named("Int"));
    let inner = anonymous(
        Vec::new(),
        Some(ty_named("Int")),
        vec![ret(Some(int_lit(42)))],
    );
    let outer = ast::Expr::AnonymousFunction {
        id: ast::AnonymousFunctionId(1),
        is_suspend: false,
        params: Vec::new(),
        return_ty: Some(ty_named("Int")),
        body: block(vec![
            val_ty("inner", Some(signature.clone()), inner),
            ret(Some(call("inner", Vec::new()))),
        ]),
        span: sp(),
    };
    let module = lower_user(file(vec![fun(
        "main",
        vec![val_ty("outer", Some(signature), outer)],
    )]))
    .expect("nested anonymous functions with the same signature must have distinct bodies");

    let names: std::collections::HashSet<_> = module
        .anonymous_functions
        .iter()
        .map(|(_, function)| module.functions[function.function].name.as_str())
        .collect();
    assert_eq!(module.anonymous_functions.len(), 2);
    assert_eq!(names.len(), 2);
    assert!(names.contains("$anonymous.0"));
    assert!(names.contains("$anonymous.1"));
    assert!(module.anonymous_functions.iter().all(|(_, function)| {
        function.definition_root == hir::LexicalDefinitionRoot::Function(module.entry())
    }));
    let paths = module
        .anonymous_functions
        .iter()
        .map(|(_, function)| definition_path(&function.definition_path))
        .collect::<Vec<_>>();
    assert!(paths.contains(&vec![(
        scoop_identity::StructuralDefinitionSiteRole::Lambda,
        0,
    )]));
    assert!(paths.contains(&vec![
        (scoop_identity::StructuralDefinitionSiteRole::Lambda, 0),
        (scoop_identity::StructuralDefinitionSiteRole::Lambda, 0),
    ]));
}

#[test]
fn suspend_lambda_owns_a_suspend_body_and_is_callable_only_in_suspend_context() {
    let signature = ty_function(true, Vec::new(), ty_named("Int"));
    let module = lower_user(file(vec![
        suspend_fun(
            "run",
            vec![
                val_ty(
                    "task",
                    Some(signature),
                    suspend_lambda(Some(Vec::new()), int_lit(42)),
                ),
                stmt(call("task", Vec::new())),
            ],
        ),
        fun("main", Vec::new()),
    ]))
    .expect("a suspend function may invoke a suspend lambda value");

    let (_, lambda) = module.lambdas.iter().next().expect("suspend lambda");
    assert!(module.function_types[lambda.function_type].is_suspend);
    assert!(module.functions[lambda.function].is_suspend);
    let run = module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == "run").then_some(function))
        .expect("run function");
    let hir::FunctionKind::User(body) = &run.kind else {
        panic!("run body");
    };
    assert!(matches!(
        body.statements[1].kind,
        hir::StatementKind::Expr(hir::Expr {
            kind: hir::ExprKind::CallableCall { .. },
            ..
        })
    ));

    let errors = lower_user(file(vec![fun(
        "main",
        vec![
            val_ty(
                "task",
                Some(ty_function(true, Vec::new(), ty_named("Int"))),
                suspend_lambda(Some(Vec::new()), int_lit(42)),
            ),
            stmt(call("task", Vec::new())),
        ],
    )]))
    .expect_err("ordinary code must not invoke a suspend function value");
    assert!(errors.iter().any(|error| {
        error
            .message
            .contains("suspend function value cannot be called from non-suspend function `main`")
    }));
}
