use super::*;

fn labelled_return(name: &str, value: ast::Expr) -> ast::Statement {
    ast::Statement {
        kind: ast::StatementKind::Return {
            label: Some(ident(name)),
            value: Some(value),
        },
        span: sp(),
    }
}

#[test]
fn lambda_inference_combines_early_returns_with_the_tail() {
    let operation = ast::Expr::Lambda {
        label: Some(ident("leave")),
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters: None,
        body: block(vec![
            if_stmt(
                bool_lit(true),
                vec![labelled_return("leave", int_lit(1))],
                None,
            ),
            stmt(int_lit(2)),
        ]),
        span: sp(),
    };
    let module = lower_user(file(vec![fun("main", vec![val("operation", operation)])])).unwrap();
    let (_, lambda) = module.lambdas.iter().next().unwrap();
    assert_eq!(
        module.function_types[lambda.function_type].return_type,
        int_type(&module)
    );
}

#[test]
fn named_local_function_return_does_not_use_an_enclosing_lambda_label() {
    let operation = ast::Expr::Lambda {
        label: Some(ident("leave")),
        id: ast::LambdaId(0),
        is_suspend: false,
        parameters: None,
        body: block(vec![
            local_fun_sig(
                "local",
                Vec::new(),
                Vec::new(),
                Some(ty_named("Int")),
                vec![labelled_return("local", int_lit(7))],
            ),
            labelled_return("leave", call("local", Vec::new())),
        ]),
        span: sp(),
    };
    let module = lower_user(file(vec![fun("main", vec![val("operation", operation)])])).unwrap();
    let (_, lambda) = module.lambdas.iter().next().unwrap();
    assert_eq!(
        module.function_types[lambda.function_type].return_type,
        int_type(&module)
    );
}
