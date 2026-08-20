use super::*;

#[test]
fn expression_results_follow_definition_dependencies_and_generic_parameters() {
    let module = lower_user(file(vec![
        fun_expr(
            "start",
            vec![],
            vec![],
            None,
            call("identity", vec![int_lit(7)]),
        ),
        fun_expr(
            "identity",
            vec!["T"],
            vec![("value", ty_named("T"))],
            None,
            var("value"),
        ),
        fun("main", vec![val("value", call("start", vec![]))]),
    ]))
    .unwrap();
    let start = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "start")
        .unwrap()
        .1;
    assert_eq!(start.return_ty, int_type(&module));
    let identity = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "identity")
        .unwrap()
        .1;
    assert!(matches!(
        module.types[identity.return_ty],
        hir::Type::Param(_)
    ));
}

#[test]
fn a_recursive_result_cannot_fall_back_to_a_less_specific_overload() {
    let errors = lower_user(file(vec![
        fun_expr(
            "choose",
            vec![],
            vec![("value", ty_named("Int"))],
            None,
            call("choose", vec![int_lit(1)]),
        ),
        fun_expr(
            "choose",
            vec![],
            vec![("value", ty_named("Any"))],
            None,
            int_lit(2),
        ),
        fun("main", vec![]),
    ]))
    .unwrap_err();
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("recursive declaration type inference")),
        "{errors:?}"
    );
}

#[test]
fn an_inapplicable_recursive_spelling_does_not_create_a_dependency() {
    let module = lower_user(file(vec![
        fun_expr(
            "choose",
            vec![],
            vec![("value", ty_named("Int"))],
            None,
            call("choose", vec![str_lit("next")]),
        ),
        fun_expr(
            "choose",
            vec![],
            vec![("value", ty_named("String"))],
            None,
            int_lit(2),
        ),
        fun("main", vec![]),
    ]))
    .unwrap();
    assert!(
        module
            .functions
            .iter()
            .filter(|(_, function)| function.name == "choose")
            .all(|(_, function)| function.return_ty == int_type(&module))
    );
}

#[test]
fn an_explicit_result_breaks_a_mutual_inference_cycle() {
    lower_user(file(vec![
        fun_expr("first", vec![], vec![], None, call("second", vec![])),
        fun_expr(
            "second",
            vec![],
            vec![],
            Some(ty_named("Int")),
            call("first", vec![]),
        ),
        fun("main", vec![]),
    ]))
    .unwrap();
}

#[test]
fn inferred_local_functions_keep_their_declaration_captures() {
    let ast::Decl::Function(local) = fun_expr("read", vec![], vec![], None, var("captured")) else {
        unreachable!()
    };
    let module = lower_user(file(vec![fun(
        "main",
        vec![
            val("captured", int_lit(9)),
            ast::Statement {
                kind: ast::StatementKind::LocalFunction(local),
                span: sp(),
            },
            val("value", call("read", vec![])),
        ],
    )]))
    .unwrap();
    let local = module.local_functions.iter().next().unwrap().1;
    assert_eq!(
        module.function_types[local.function_type].return_type,
        int_type(&module)
    );
    assert_eq!(local.captures.len(), 1);
}
