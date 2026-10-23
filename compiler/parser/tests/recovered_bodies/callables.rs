use super::*;

#[test]
fn callable_headers_keep_their_actual_parameters_and_body_parent() {
    for text in [
        "fun editing() { val action = { value: String -> value. } }",
        "fun editing() { val action = fun(value: String): Boolean { value. } }",
    ] {
        let parsed = parse_recovering(text).unwrap();
        let body = body(&parsed, "editing");
        let [literal] = body.callable_literals.as_slice() else {
            panic!("one literal: {body:?}")
        };
        let block = body
            .blocks
            .iter()
            .find(|block| block.id == literal.body)
            .unwrap();
        assert_eq!(block.parent, literal.block);
        assert_eq!(body.selectors[0].block, Some(literal.body));
        match &literal.header {
            ast::RecoveredCallableHeader::Lambda { parameters } => {
                let [parameter] = parameters.as_deref().unwrap() else {
                    panic!("one parameter")
                };
                assert_eq!(source(text, parameter.span), "value: String");
                assert_eq!(block.scope, ast::RecoveredBlockScope::Lambda);
            }
            ast::RecoveredCallableHeader::AnonymousFunction {
                parameters,
                return_ty,
                receiver,
            } => {
                assert!(receiver.is_none());
                assert_eq!(source(text, parameters[0].span), "value: String");
                assert_eq!(source(text, return_ty.as_ref().unwrap().span), "Boolean");
                assert_eq!(block.scope, ast::RecoveredBlockScope::AnonymousFunction);
            }
        }
        assert!(parsed.ast().is_none());
        assert_eq!(parsed.syntax().text().to_string(), text);
    }
}

#[test]
fn explicit_labels_and_suspend_modifiers_survive_an_unclosed_literal() {
    for expression in [
        "内部@ suspend { value: String -> value.",
        "内部@ suspend fun String.(): Boolean { this.",
    ] {
        let text = format!("fun editing() {{ val action = {expression}");
        let parsed = parse_recovering(&text).unwrap();
        let body = body(&parsed, "editing");
        let [literal] = body.callable_literals.as_slice() else {
            panic!("one literal")
        };
        assert!(literal.is_suspend);
        assert_eq!(literal.label.as_ref().unwrap().text, "内部");
        assert_eq!(literal.span.start as usize, text.find("内部").unwrap());
        assert!(
            !body
                .blocks
                .iter()
                .find(|block| block.id == literal.body)
                .unwrap()
                .closed
        );
        assert_eq!(parsed.syntax().text().to_string(), text);
    }
}

#[test]
fn nested_literal_headers_stay_with_their_own_blocks() {
    let text = "fun editing() { val outer = fun(value: String) { val inner = { flag: Boolean -> flag. } } }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    assert_eq!(body.callable_literals.len(), 2);
    let outer = body
        .callable_literals
        .iter()
        .find(|literal| {
            matches!(
                literal.header,
                ast::RecoveredCallableHeader::AnonymousFunction { .. }
            )
        })
        .unwrap();
    let inner = body
        .callable_literals
        .iter()
        .find(|literal| matches!(literal.header, ast::RecoveredCallableHeader::Lambda { .. }))
        .unwrap();
    assert_eq!(inner.block, Some(outer.body));
    assert_eq!(body.selectors[0].block, Some(inner.body));
}

#[test]
fn a_destructured_parameter_keeps_one_complete_tuple_subject() {
    let text = "fun editing() { val action = { (first, second): (String, Boolean), extra: Boolean -> first. } }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    let ast::RecoveredCallableHeader::Lambda {
        parameters: Some(parameters),
    } = &body.callable_literals[0].header
    else {
        panic!("explicit lambda header")
    };
    assert_eq!(parameters.len(), 2);
    assert!(matches!(parameters[0].target, ast::Pattern::Tuple { .. }));
    assert!(matches!(
        parameters[0].ty.as_ref().unwrap().kind,
        ast::TypeRefKind::Tuple(_)
    ));
    assert_eq!(body.selectors.len(), 1);
}

#[test]
fn failed_parameter_probes_restore_the_implicit_lambda_header() {
    let text = "fun editing(value: String) { val action = { value. } }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    assert_eq!(body.callable_literals.len(), 1);
    assert!(matches!(
        body.callable_literals[0].header,
        ast::RecoveredCallableHeader::Lambda { parameters: None }
    ));
    assert_eq!(body.selectors.len(), 1);
}

#[test]
fn complete_literals_or_unknown_headers_do_not_produce_recovered_parameters() {
    for text in [
        "fun editing(value: String) { val action = { input: String -> input }; value. }",
        "fun editing(value: String) { val action = fun(input: String): String { return input }; value. }",
        "fun editing() { val action = fun(value: ) { value. } }",
        "fun editing() { val action = { value: -> value. } }",
        "fun editing() { val action = fun(value: String) value. }",
    ] {
        let parsed = parse_recovering(text).unwrap();
        assert!(
            body(&parsed, "editing").callable_literals.is_empty(),
            "{text}"
        );
    }
}
