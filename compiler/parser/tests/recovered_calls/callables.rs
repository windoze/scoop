use super::*;

#[test]
fn callable_argument_slots_reference_their_actual_header_and_body() {
    let text = "fun editing() { 调用(true, 参数 = { 值 -> 值. }, fun(值: String) { 值. }) }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    let [call] = body.calls.as_slice() else {
        panic!("one call")
    };
    assert!(matches!(
        call.arguments.as_ref().unwrap().items[0].value,
        Value::Complete(_)
    ));
    assert_eq!(body.callable_literals.len(), 2);
    for argument in &call.arguments.as_ref().unwrap().items[1..] {
        let Value::CallableLiteral(id) = argument.value else {
            panic!("actual callable argument: {argument:?}")
        };
        let literal = body
            .callable_literals
            .iter()
            .find(|literal| literal.body == id)
            .unwrap();
        assert_eq!(literal.block, call.block);
        assert!(argument.span.start <= literal.span.start);
        assert_eq!(argument.span.end, literal.span.end);
        assert!(
            body.selectors
                .iter()
                .any(|selector| selector.block == Some(id))
        );
    }
    assert!(
        matches!(&call.arguments.as_ref().unwrap().items[1].name, ast::CallArgumentName::Named(name) if name.text == "参数")
    );
    assert_eq!(
        body.callable_literals[0].label.as_ref().unwrap().text,
        "调用"
    );
    assert!(body.callable_literals[1].label.is_none());
    assert!(parsed.ast().is_none());
    assert_eq!(parsed.syntax().text().to_string(), text);
}

#[test]
fn explicit_labels_take_precedence_over_named_call_labels() {
    for (expression, label) in [
        ("调用({ 值 -> 值. })", Some("调用")),
        ("host.调用({ 值 -> 值. })", Some("调用")),
        ("调用(内部@ { 值 -> 值. })", Some("内部")),
        ("(调用)({ 值 -> 值. })", None),
        ("调用(fun(值: String) { 值. })", None),
    ] {
        let text = format!("fun editing() {{ {expression} }}");
        let parsed = parse_recovering(&text).unwrap();
        let body = body(&parsed, "editing");
        assert!(matches!(
            body.calls[0].arguments.as_ref().unwrap().items[0].value,
            Value::CallableLiteral(_)
        ));
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
fn an_unclosed_callable_argument_keeps_its_source_slot_at_eof() {
    let text = "fun editing() { call(true, block = suspend { value -> value.";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    let call = &body.calls[0];
    assert!(!call.arguments.as_ref().unwrap().closed);
    assert_eq!(call.arguments.as_ref().unwrap().items.len(), 2);
    let Value::CallableLiteral(id) = call.arguments.as_ref().unwrap().items[1].value else {
        panic!("literal at EOF")
    };
    let literal = &body.callable_literals[0];
    assert_eq!(literal.body, id);
    assert!(literal.is_suspend);
    assert!(
        !body
            .blocks
            .iter()
            .find(|block| block.id == id)
            .unwrap()
            .closed
    );
    assert_eq!(call.arguments.as_ref().unwrap().end as usize, text.len());
}

#[test]
fn nested_call_arguments_keep_separate_literal_associations() {
    let text = "fun editing() { outer({ first -> inner({ second -> second. }) }) }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    assert_eq!(body.calls.len(), 2);
    for call in &body.calls {
        let Value::CallableLiteral(id) = call.arguments.as_ref().unwrap().items[0].value else {
            panic!("nested literal")
        };
        let literal = body
            .callable_literals
            .iter()
            .find(|literal| literal.body == id)
            .unwrap();
        let Target::Named(name) = &call.target else {
            panic!("named call")
        };
        assert_eq!(literal.block, call.block);
        assert_eq!(literal.label.as_ref().unwrap().text, name.text);
    }
}

#[test]
fn complete_literals_and_invalid_headers_do_not_create_callable_recovery_slots() {
    for (expression, complete) in [
        ("call({ value: String -> value }, )", true),
        ("call(fun(value: String) { return value }, )", true),
        ("call(fun(value: ) { value. })", false),
        ("call({ value: -> value. })", false),
    ] {
        let text = format!("fun editing() {{ {expression} }}");
        let parsed = parse_recovering(&text).unwrap();
        let body = body(&parsed, "editing");
        assert!(body.callable_literals.is_empty(), "{text}");
        assert_eq!(
            matches!(
                body.calls[0].arguments.as_ref().unwrap().items[0].value,
                Value::Complete(_)
            ),
            complete,
            "{text}"
        );
    }
}
