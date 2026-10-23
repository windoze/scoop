use super::*;

#[test]
fn while_recovery_keeps_the_original_label_condition_and_block() {
    let text = "fun editing(value: Any) { 重复@ while (value is String) { value. } }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    assert_eq!(body.while_loops.len(), 1);
    let while_loop = &body.while_loops[0];
    assert_eq!(while_loop.start as usize, text.find("重复").unwrap());
    assert_eq!(while_loop.label.as_ref().unwrap().text, "重复");
    assert_eq!(source(text, while_loop.condition.span()), "value is String");
    let ast::RecoveredControlBody::Block(id) = while_loop.body else {
        panic!("the braced loop body retains its opening token");
    };
    assert_eq!(body.selectors[0].block, Some(id));
    assert_eq!(
        body.blocks
            .iter()
            .find(|block| block.id == id)
            .unwrap()
            .parent,
        while_loop.block
    );
    assert_eq!(parsed.syntax().text().to_string(), text);
}

#[test]
fn single_statement_and_nested_loops_keep_their_actual_scopes() {
    let text =
        "fun editing(value: Any) { outer@ while (true) inner@ while (value is String) value. }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    assert_eq!(body.blocks.len(), 1);
    assert_eq!(body.while_loops.len(), 2);
    for while_loop in &body.while_loops {
        let start = match while_loop.label.as_ref().unwrap().text.as_str() {
            "outer" => text.find("inner@").unwrap(),
            "inner" => text.rfind("value.").unwrap(),
            label => panic!("unexpected label {label}"),
        };
        assert_eq!(
            while_loop.body,
            ast::RecoveredControlBody::Statement(start as u32)
        );
        assert_eq!(while_loop.block, body.selectors[0].block);
    }
}

#[test]
fn incomplete_while_conditions_do_not_make_a_complete_loop_header() {
    for text in [
        "fun editing(value: Any) { cycle@ while (value.) { value. } }",
        "fun editing(value: Any) { cycle@ while (check(,)) { value. } }",
    ] {
        let parsed = parse_recovering(text).unwrap();
        let body = body(&parsed, "editing");
        assert!(body.while_loops.is_empty(), "{text}");
        let [operand] = body.operands.as_slice() else {
            panic!("one unfinished condition: {body:?}");
        };
        assert_eq!(
            operand.statement_start as usize,
            text.find("cycle@").unwrap()
        );
        let ast::RecoveredOperandContext::WhileCondition { label } = &operand.context else {
            panic!("a loop condition keeps its control context");
        };
        assert_eq!(label.as_ref().unwrap().text, "cycle");
    }
}

#[test]
fn complete_while_loops_do_not_copy_their_condition_ast() {
    let parsed = parse_recovering(
        "fun editing(value: Boolean) { cycle@ while (value) { break@cycle }; value. }",
    )
    .unwrap();
    let body = body(&parsed, "editing");
    assert!(body.while_loops.is_empty());
    assert!(body.operands.is_empty());
}
