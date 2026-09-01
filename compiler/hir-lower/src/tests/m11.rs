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

#[test]
fn callable_literal_inherits_the_enclosing_generic_namespace() {
    let result = ty_function(false, Vec::new(), ty_named("T"));
    let module = lower_user(file(vec![
        fun_expr(
            "make",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(result),
            lambda(Some(Vec::new()), var("value")),
        ),
        fun("main", vec![]),
    ]))
    .expect("a callable literal may retain the enclosing type parameter");

    let (_, lambda) = module.lambdas.iter().next().expect("lambda entity");
    assert_eq!(lambda.owner_type_param_count, 1);
    assert_eq!(module.functions[lambda.function].type_params()[0].name, "T");
    assert!(matches!(
        module.functions[lambda.function].genericity,
        hir::FunctionGenericity::Generic { .. }
    ));
    assert!(matches!(
        module.types[lambda.captures[0].ty],
        hir::Type::Param(_)
    ));
}

#[test]
fn function_variance_builds_an_explicit_typed_coercion() {
    let source_type = ty_function(false, vec![ty_named("Any")], ty_named("Int"));
    let target_type = ty_function(false, vec![ty_named("Int")], ty_named("Any"));
    let reference = ast::Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: None,
        name: ident("source"),
        span: sp(),
    };
    let module = lower_user(file(vec![
        fun_expr(
            "source",
            Vec::new(),
            vec![("value", ty_named("Any"))],
            Some(ty_named("Int")),
            int_lit(42),
        ),
        fun(
            "main",
            vec![
                val_ty("exact", Some(source_type), reference),
                val_ty("widened", Some(target_type), var("exact")),
            ],
        ),
    ]))
    .expect("function parameters are contravariant and returns are covariant");

    let hir::FunctionKind::User(body) = &module.functions[module.entry].kind else {
        panic!("main body");
    };
    let hir::StatementKind::ValDecl { init, .. } = &body.statements[1].kind else {
        panic!("widened declaration");
    };
    let hir::ExprKind::FunctionCoercion {
        source,
        coercion,
        target_type,
    } = &init.kind
    else {
        panic!("variance must not be a pointer-only retype");
    };
    let conversion = &module.function_coercions[*coercion];
    assert_eq!(conversion.target, *target_type);
    assert!(matches!(source.kind, hir::ExprKind::Local(_)));
    assert_eq!(module.function_coercions.len(), 1);
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

fn suspend_lambda(parameters: Option<Vec<ast::LambdaParam>>, tail: ast::Expr) -> ast::Expr {
    let ast::Expr::Lambda {
        id,
        parameters,
        body,
        span,
        ..
    } = lambda(parameters, tail)
    else {
        unreachable!("lambda helper returns a lambda")
    };
    ast::Expr::Lambda {
        id,
        is_suspend: true,
        parameters,
        body,
        span,
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
            kind: hir::ExprKind::Binary { .. },
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

#[test]
fn suspend_callable_reference_preserves_suspend_identity() {
    let reference = ast::Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: None,
        name: ident("waitForValue"),
        span: sp(),
    };
    let module = lower_user(file(vec![
        suspend_fun("waitForValue", Vec::new()),
        fun(
            "main",
            vec![val_ty(
                "task",
                Some(ty_function(true, Vec::new(), ty_named("Unit"))),
                reference,
            )],
        ),
    ]))
    .expect("creating a suspend callable reference is synchronous");
    let (_, reference) = module
        .callable_references
        .iter()
        .next()
        .expect("callable reference");
    assert!(module.function_types[reference.function_type].is_suspend);
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
    let hir::CallableReferenceTarget::Named(callable) = &reference.target else {
        panic!("expected a top-level callable reference")
    };
    let target = module.callable_function(*callable);
    assert_eq!(module.functions[target].name, "increment");
}

#[test]
fn generic_top_level_reference_is_fixed_by_its_expected_type() {
    let signature = ty_function(false, vec![ty_named("Int")], ty_named("Int"));
    let reference = ast::Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: None,
        name: ident("identity"),
        span: sp(),
    };
    let module = lower_user(file(vec![
        fun_expr(
            "identity",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(ty_named("T")),
            var("value"),
        ),
        fun(
            "main",
            vec![val_ty("operation", Some(signature), reference)],
        ),
    ]))
    .expect("the expected function type should fix the generic reference");

    let (_, reference) = module
        .callable_references
        .iter()
        .next()
        .expect("reference entity");
    let hir::CallableReferenceTarget::Named(hir::Callable::Generic(resolved)) = &reference.target
    else {
        panic!("expected a resolved generic reference")
    };
    assert_eq!(module.instantiations[*resolved].type_args, vec![module.int]);
}

#[test]
fn bound_reference_retains_receiver_and_resolved_member_identity() {
    let operation_ty = ty_function(false, vec![ty_named("Int")], ty_named("Int"));
    let reference = ast::Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: Some(Box::new(var("mapper"))),
        name: ident("map"),
        span: sp(),
    };
    let module = lower_user(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Mapper",
            vec![(false, "offset", ty_named("Int"))],
            None,
            vec![],
            vec![method_expr(
                "map",
                vec![("value", ty_named("Int"))],
                Some(ty_named("Int")),
                binary(ast::BinOp::Add, var("value"), var("offset")),
            )],
        ),
        fun(
            "main",
            vec![
                val("mapper", call("Mapper", vec![int_lit(2)])),
                val_ty("operation", Some(operation_ty), reference),
            ],
        ),
    ]))
    .expect("a bound member reference should lower");

    let (_, reference) = module
        .callable_references
        .iter()
        .next()
        .expect("reference entity");
    let hir::CallableReferenceTarget::BoundMember { receiver, callee } = &reference.target else {
        panic!("expected a bound member target")
    };
    assert!(matches!(receiver.kind, hir::ExprKind::Local(_)));
    let target = module.callable_function(*callee);
    assert_eq!(module.functions[target].name, "Mapper.map");
    assert!(reference.captures.is_empty());
}

#[test]
fn extension_receiver_is_a_typed_this_parameter_and_direct_call_argument() {
    let module = lower_user(file(vec![
        extension_expr(
            ty_named("Int"),
            "bump",
            vec![],
            vec![("delta", ty_named("Int"))],
            Some(ty_named("Int")),
            binary(ast::BinOp::Add, this_expr(), var("delta")),
        ),
        fun(
            "main",
            vec![val(
                "result",
                method_call(int_lit(40), "bump", vec![int_lit(2)]),
            )],
        ),
    ]))
    .expect("extension call should lower");

    let extension = module
        .top_level
        .iter()
        .map(|id| &module.functions[*id])
        .find(|function| function.name == "bump")
        .expect("extension function");
    assert!(extension.method.is_none());
    assert_eq!(extension.params.len(), 2);
    assert_eq!(extension.params[0].name, "this");
    assert_eq!(extension.params[0].ty, module.int);

    let hir::FunctionKind::User(main) = &module.functions[module.entry].kind else {
        panic!("main body")
    };
    let hir::StatementKind::ValDecl { init, .. } = &main.statements[0].kind else {
        panic!("result declaration")
    };
    let hir::ExprKind::Call { callee, args } = &init.kind else {
        panic!("extension invocation must be a direct call")
    };
    assert_eq!(
        module.functions[module.callable_function(*callee)].name,
        "bump"
    );
    assert_eq!(args.len(), 2);
    assert_eq!(args[0].ty, module.int);
}

#[test]
fn bound_extension_reference_has_a_distinct_direct_target() {
    let operation_ty = ty_function(false, vec![ty_named("String")], ty_named("String"));
    let reference = ast::Expr::CallableReference {
        id: ast::CallableReferenceId(0),
        receiver: Some(Box::new(str_lit("kept"))),
        name: ident("decorate"),
        span: sp(),
    };
    let module = lower_user(file(vec![
        extension_expr(
            ty_named("String"),
            "decorate",
            vec![],
            vec![("suffix", ty_named("String"))],
            Some(ty_named("String")),
            binary(ast::BinOp::Add, this_expr(), var("suffix")),
        ),
        fun(
            "main",
            vec![val_ty("operation", Some(operation_ty), reference)],
        ),
    ]))
    .expect("bound extension reference should lower");

    let (_, reference) = module
        .callable_references
        .iter()
        .next()
        .expect("reference entity");
    let hir::CallableReferenceTarget::BoundExtension { receiver, callee } = &reference.target
    else {
        panic!("expected a bound extension target")
    };
    assert_eq!(receiver.ty, module.string);
    assert_eq!(
        module.functions[module.callable_function(*callee)].name,
        "decorate"
    );
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

#[test]
fn local_function_has_typed_identity_capture_and_lifted_direct_call() {
    let module = lower_user(file(vec![fun(
        "main",
        vec![
            val("base", int_lit(2)),
            local_fun_sig(
                "add",
                vec![],
                vec![("value", ty_named("Int"))],
                Some(ty_named("Int")),
                vec![ret(Some(binary(
                    ast::BinOp::Add,
                    var("value"),
                    var("base"),
                )))],
            ),
            val("result", call("add", vec![int_lit(40)])),
        ],
    )]))
    .expect("capturing local function should lower");

    assert_eq!(module.local_functions.len(), 1);
    let (local_id, local) = module
        .local_functions
        .iter()
        .next()
        .expect("local function");
    assert_eq!(local.captures.len(), 1);
    assert_eq!(local.captures[0].name, "base");
    let hir::FunctionKind::User(main_body) = &module.functions[module.entry].kind else {
        panic!("main body")
    };
    assert!(matches!(
        main_body.statements[1].kind,
        hir::StatementKind::LocalFunction(id) if id == local_id
    ));
    let hir::StatementKind::ValDecl { init, .. } = &main_body.statements[2].kind else {
        panic!("result declaration")
    };
    assert!(matches!(
        init.kind,
        hir::ExprKind::LocalFunctionCall {
            local_function,
            ref captures,
            ..
        } if local_function == local_id && captures.len() == 1
    ));
    let lifted = &module.functions[local.function];
    assert_eq!(
        lifted.params.len(),
        2,
        "capture parameter precedes source parameter"
    );
}

#[test]
fn overload_probes_lambda_candidates_transactionally() {
    let int_operation = ty_function(false, vec![ty_named("Int")], ty_named("Int"));
    let string_operation = ty_function(false, vec![ty_named("Int")], ty_named("String"));
    let operation = lambda(
        Some(vec![ast::LambdaParam {
            target: pat_bind("value"),
            ty: None,
            span: sp(),
        }]),
        str_lit("selected"),
    );
    let module = lower_user(file(vec![
        fun_expr(
            "select",
            vec![],
            vec![("operation", int_operation)],
            Some(ty_named("Int")),
            int_lit(0),
        ),
        fun_expr(
            "select",
            vec![],
            vec![("operation", string_operation)],
            Some(ty_named("String")),
            str_lit(""),
        ),
        fun("main", vec![val("result", call("select", vec![operation]))]),
    ]))
    .expect("the lambda result should leave exactly one applicable overload");

    assert_eq!(
        module.lambdas.len(),
        1,
        "discarded candidate probes must not leak lambda entities"
    );
    let hir::FunctionKind::User(main) = &module.functions[module.entry].kind else {
        panic!("main body")
    };
    let hir::StatementKind::ValDecl { init, .. } = &main.statements[0].kind else {
        panic!("result declaration")
    };
    let hir::ExprKind::Call { callee, .. } = &init.kind else {
        panic!("resolved overload call")
    };
    assert_eq!(
        module.functions[module.callable_function(*callee)].return_ty,
        module.string
    );
}
