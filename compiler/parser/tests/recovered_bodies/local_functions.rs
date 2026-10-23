use super::*;

#[test]
fn local_function_headers_keep_the_declaring_block_and_their_own_body() {
    for expression in [
        "fun local(value: String) { value. }",
        "fun local(value: String) = value.",
    ] {
        let text = format!("fun editing() {{ {expression} }}");
        let parsed = parse_recovering(&text).unwrap();
        let outer = body(&parsed, "editing");
        let [local] = outer.local_functions.as_slice() else {
            panic!("one local header: {outer:?}")
        };
        assert_eq!(local.block, Some(outer.blocks[0].id));
        assert_eq!(local.header.signature.name.text, "local");
        assert_eq!(local.body.as_ref(), body(&parsed, "local"));
        assert!(outer.selectors.is_empty());
        assert_eq!(local.body.selectors.len(), 1);
        assert!(parsed.ast().is_none());
        assert_eq!(parsed.syntax().text().to_string(), text);
    }
}

#[test]
fn nested_local_declarations_keep_lexical_ownership_through_a_lambda() {
    let text = "fun editing() { val action = { fun first(value: String) { fun second(flag: Boolean) { value.";
    let parsed = parse_recovering(text).unwrap();
    let outer = body(&parsed, "editing");
    let first = &outer.local_functions[0];
    assert_eq!(first.block, Some(outer.callable_literals[0].body));
    let second = &first.body.local_functions[0];
    assert_eq!(second.header.signature.name.text, "second");
    assert_eq!(second.block, Some(first.body.blocks[0].id));
    assert_eq!(second.body.selectors.len(), 1);
    assert!(!second.body.blocks[0].closed);
}

#[test]
fn complete_local_functions_and_failed_headers_are_not_recovered_as_valid_heads() {
    for declaration in [
        "fun local(value: String): String = value",
        "fun local(value: ) { value. }",
    ] {
        let text = format!("fun editing(value: Boolean) {{ {declaration}; value. }}");
        let parsed = parse_recovering(&text).unwrap();
        assert!(
            body(&parsed, "editing").local_functions.is_empty(),
            "{text}"
        );
    }
}

#[test]
fn adjacent_local_declarations_keep_source_order_and_member_functions_stay_separate() {
    let text = "class Host { fun editing() { if (true) { fun first(value: String) { value. } } else { fun second(value: Boolean) { value. } } } }";
    let parsed = parse_recovering(text).unwrap();
    let outer = body(&parsed, "editing");
    assert_eq!(
        outer
            .local_functions
            .iter()
            .map(|function| function.header.signature.name.text.as_str())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert_ne!(
        outer.local_functions[0].block,
        outer.local_functions[1].block
    );
}
