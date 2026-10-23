use super::*;

#[test]
fn trailing_calls_keep_only_the_parentheses_that_exist_in_source() {
    for (expression, parenthesized, count) in [
        ("call { value -> value. }", false, 0),
        ("call() { value -> value. }", true, 0),
        ("call(true, named = false) { value -> value. }", true, 2),
        ("call<String> { value -> value. }", false, 0),
    ] {
        let text = format!("fun editing() {{ {expression} }}");
        let parsed = parse_recovering(&text).unwrap();
        let body = body(&parsed, "editing");
        let [call] = body.calls.as_slice() else {
            panic!("one actual call: {body:?}")
        };
        assert_eq!(call.arguments.is_some(), parenthesized);
        if let Some(arguments) = &call.arguments {
            assert_eq!(source(&text, arguments.opening), "(");
            assert_eq!(arguments.items.len(), count);
            assert!(arguments.closed);
            assert!(
                arguments
                    .items
                    .iter()
                    .all(|argument| matches!(argument.value, Value::Complete(_)))
            );
        }
        let trailing = call.trailing.as_ref().unwrap();
        let Value::CallableLiteral(id) = trailing.value else {
            panic!("actual trailing literal")
        };
        assert!(matches!(
            trailing.name,
            ast::CallArgumentName::TrailingLambda
        ));
        assert_eq!(id, body.callable_literals[0].body);
        assert_eq!(call.span.end, trailing.span.end);
        assert_eq!(source(&text, trailing.span), "{ value -> value. }");
        assert!(parsed.ast().is_none());
        assert_eq!(parsed.syntax().text().to_string(), text);
    }
}

#[test]
fn grouping_receivers_generics_and_labels_keep_the_original_call_forms() {
    for (expression, named, label) in [
        ("host?.call<String> { value -> value. }", true, Some("call")),
        (
            "call(true) 内部@ suspend { value -> value. }",
            true,
            Some("内部"),
        ),
        ("super.call { value -> value. }", true, Some("call")),
        ("super<Base>.call { value -> value. }", true, Some("call")),
        ("(factory()) { value -> value. }", false, None),
        ("(action) { value -> value. }", false, None),
    ] {
        let text = format!("fun editing() {{ {expression} }}");
        let parsed = parse_recovering(&text).unwrap();
        let body = body(&parsed, "editing");
        assert_eq!(body.calls.len(), 1, "{text}");
        assert_eq!(
            !matches!(body.calls[0].target, Target::Invoke(_)),
            named,
            "{text}"
        );
        assert_eq!(
            body.callable_literals[0]
                .label
                .as_ref()
                .map(|name| name.text.as_str()),
            label,
            "{text}"
        );
    }
}

#[test]
fn complete_trailing_calls_do_not_publish_recovery_duplicates() {
    for expression in [
        "call { true }",
        "call(true) { false }",
        "call<String> 内部@ { return@内部 true }",
        "host?.call { true }",
        "(factory()) { true }",
        "call { nested { true } }",
    ] {
        let text = format!("fun editing(value: String) {{ {expression}; value. }}");
        let parsed = parse_recovering(&text).unwrap();
        let body = body(&parsed, "editing");
        assert!(body.calls.is_empty(), "{text}: {:?}", body.calls);
        assert!(body.callable_literals.is_empty(), "{text}");
        assert_eq!(body.selectors.len(), 1);
    }
}

#[test]
fn nested_trailing_inputs_and_eof_retain_each_real_body() {
    let text = "fun editing() { outer { first -> inner(first) { second -> second.";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    assert_eq!(body.calls.len(), 2);
    for call in &body.calls {
        let Value::CallableLiteral(id) = call.trailing.as_ref().unwrap().value else {
            panic!("nested trailing argument")
        };
        let literal = body
            .callable_literals
            .iter()
            .find(|literal| literal.body == id)
            .unwrap();
        assert_eq!(literal.block, call.block);
        assert_eq!(call.span.end as usize, text.len());
        assert!(
            !body
                .blocks
                .iter()
                .find(|block| block.id == id)
                .unwrap()
                .closed
        );
    }
}

#[test]
fn malformed_trailing_headers_and_second_lambdas_remain_syntax_errors() {
    for expression in [
        "call { value: -> value. }",
        "call { true } { value -> value. }",
    ] {
        let text = format!("fun editing() {{ {expression} }}");
        let parsed = parse_recovering(&text).unwrap();
        assert!(parsed.ast().is_none());
        assert!(!parsed.diagnostics().is_empty());
        assert!(
            body(&parsed, "editing").callable_literals.is_empty(),
            "{text}"
        );
    }
}
