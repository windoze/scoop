use super::*;

#[test]
fn for_recovery_retains_complete_patterns_iterables_and_labels() {
    let text = "fun editing(values: Iterable<(Boolean, String)>) { 遍历@ for ((flag, text) in values) { text. } }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    let [for_loop] = body.for_loops.as_slice() else {
        panic!("one recovered loop: {body:?}");
    };
    assert_eq!(for_loop.label.as_ref().unwrap().text, "遍历");
    assert_eq!(for_loop.span.start as usize, text.find("遍历").unwrap());
    let ast::Pattern::Tuple { span, .. } = for_loop.pattern.as_ref() else {
        panic!("the complete header retains its tuple binding pattern");
    };
    assert_eq!(source(text, *span), "(flag, text)");
    assert_eq!(source(text, for_loop.iterable.span()), "values");
    let ast::RecoveredControlBody::Block(id) = for_loop.body else {
        panic!("a braced iteration body keeps its real block");
    };
    assert_eq!(body.selectors[0].block, Some(id));
    assert_eq!(
        body.blocks
            .iter()
            .find(|block| block.id == id)
            .unwrap()
            .parent,
        for_loop.block
    );
    assert_eq!(parsed.syntax().text().to_string(), text);
}

#[test]
fn single_statement_for_bodies_do_not_create_a_block_token() {
    let text = "fun editing(values: Iterable<Boolean>) { for (value in values) value. }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    assert_eq!(body.blocks.len(), 1);
    assert_eq!(body.for_loops.len(), 1);
    assert_eq!(
        body.for_loops[0].body,
        ast::RecoveredControlBody::Statement(text.rfind("value.").unwrap() as u32)
    );
    assert_eq!(body.for_loops[0].block, body.selectors[0].block);
}

#[test]
fn incomplete_patterns_and_iterables_do_not_publish_iteration_headers() {
    for text in [
        "fun editing(values: Any) { for (value in values.) { value. } }",
        "fun editing(values: Any) { for ((first, , second) in values) { first. } }",
        "fun editing(values: Any) { for (value in missing(,)) { value. } }",
    ] {
        let parsed = parse_recovering(text).unwrap();
        let body = body(&parsed, "editing");
        assert!(body.for_loops.is_empty(), "{text}: {body:?}");
    }
    let text = "fun editing(values: Any) { loop@ for (value in values.) }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    assert_eq!(body.operands.len(), 1);
    assert_eq!(
        body.operands[0].context,
        ast::RecoveredOperandContext::ForIterable
    );
    assert_eq!(
        body.operands[0].statement_start as usize,
        text.find("loop@").unwrap()
    );
}

#[test]
fn complete_for_loops_reuse_their_ordinary_ast() {
    let parsed = parse_recovering(
        "fun editing(values: Iterable<Boolean>) { for (value in values) { value }; values. }",
    )
    .unwrap();
    let body = body(&parsed, "editing");
    assert!(body.for_loops.is_empty());
    assert!(body.operands.is_empty());
}
