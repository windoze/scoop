use super::*;

fn inferred_property(name: &str, expression: Expr) -> Decl {
    Decl::Global(ast::PropertyDecl {
        context_parameters: Vec::new(),
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: None,
        body: ast::PropertyBodySyntax::Initializer {
            expression: Box::new(expression),
            accessors: ast::AccessorSyntax::default(),
        },
        span: sp(),
    })
}

#[test]
fn property_and_function_queries_follow_actual_dependencies() {
    let module = lower_user(file(vec![
        inferred_property("answer", call("read", vec![])),
        fun_expr("read", vec![], vec![], None, var("seed")),
        inferred_property("seed", int_lit(42)),
        fun("main", vec![val("answer", var("answer"))]),
    ]))
    .unwrap();
    for (_, property) in module
        .properties
        .iter()
        .filter(|(_, property)| ["answer", "seed"].contains(&property.name.as_str()))
    {
        assert_eq!(property.ty, int_type(&module));
    }
    let read = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "read")
        .unwrap()
        .1;
    assert_eq!(read.return_ty, int_type(&module));
}

#[test]
fn mixed_property_function_inference_reports_a_cycle() {
    let errors = lower_user(file(vec![
        inferred_property("answer", call("read", vec![])),
        fun_expr("read", vec![], vec![], None, var("answer")),
        fun("main", vec![]),
    ]))
    .unwrap_err();
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("recursive declaration type inference")
            && error.message.contains("answer")
            && error.message.contains("read")),
        "{errors:?}"
    );
}

#[test]
fn const_type_inference_uses_the_existing_constant_evaluator() {
    let Decl::Global(mut constant) = inferred_property("answer", int_lit(42)) else {
        unreachable!()
    };
    constant.body = ast::PropertyBodySyntax::Const(Box::new(int_lit(42)));
    let module = lower_user(file(vec![Decl::Global(constant), fun("main", vec![])])).unwrap();
    let property = module
        .properties
        .iter()
        .find(|(_, property)| property.name == "answer")
        .unwrap()
        .1;
    assert_eq!(property.ty, int_type(&module));
    assert!(matches!(
        property.representation,
        hir::PropertyRepresentation::Const { .. }
    ));
}
