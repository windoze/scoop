use scoop_hir as hir;

use super::*;

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
    assert_eq!(signature.parameter_types, vec![int_type(&module)]);
    assert_eq!(signature.return_type, int_type(&module));
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
    let hir::FunctionKind::User(main_body) = &module.functions[module.entry()].kind else {
        panic!("main body")
    };
    assert!(matches!(
        main_body.statements[1].kind,
        hir::StatementKind::LocalFunction(id) if id == local_id
    ));
    let init = local_init(main_body, "result");
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
    let hir::FunctionKind::User(main) = &module.functions[module.entry()].kind else {
        panic!("main body")
    };
    let init = local_init(main, "result");
    let hir::ExprKind::Call { callee, .. } = &init.kind else {
        panic!("resolved overload call")
    };
    assert_eq!(
        module.functions[module.callable_function(*callee)].return_ty,
        module.string
    );
}
