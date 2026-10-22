use super::*;

#[test]
fn recovered_if_keeps_one_complete_condition_and_actual_branch_entries() {
    let text = "fun editing(value: Any) { if (value is String) { value. } else { value. } }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    assert_eq!(body.conditionals.len(), 1);
    let conditional = &body.conditionals[0];
    assert_eq!(
        source(text, conditional.condition.span()),
        "value is String"
    );
    assert_eq!(conditional.start as usize, text.find("if").unwrap());
    assert_eq!(conditional.branches.len(), 2);
    for (branch, selector) in conditional.branches.iter().zip(&body.selectors) {
        let ast::RecoveredControlBody::Block(id) = branch.body else {
            panic!("a braced branch retains its opening token");
        };
        assert_eq!(selector.block, Some(id));
        assert_eq!(
            body.blocks
                .iter()
                .find(|block| block.id == id)
                .unwrap()
                .parent,
            conditional.block
        );
    }
    assert!(conditional.branches[0].on_true);
    assert!(!conditional.branches[1].on_true);
    assert_eq!(parsed.syntax().text().to_string(), text);
}

#[test]
fn a_single_statement_branch_does_not_invent_a_block_token() {
    let text = "fun editing(value: Any) { if (value is String) value. }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    assert_eq!(body.blocks.len(), 1);
    let conditional = &body.conditionals[0];
    assert_eq!(conditional.block, body.selectors[0].block);
    assert_eq!(
        conditional.branches[0].body,
        ast::RecoveredControlBody::Statement(text.rfind("value.").unwrap() as u32)
    );
}

#[test]
fn unfinished_conditions_have_an_operand_entry_but_no_branch_facts() {
    for text in [
        "fun editing(value: Any) { if (value.) { value. } }",
        "fun editing(value: Any) { if (check(,)) { value. } }",
    ] {
        let parsed = parse_recovering(text).unwrap();
        let body = body(&parsed, "editing");
        assert!(body.conditionals.is_empty(), "{text}");
        assert_eq!(body.operands.len(), 1);
        assert_eq!(
            body.operands[0].context,
            ast::RecoveredOperandContext::IfCondition
        );
    }
}

#[test]
fn complete_conditionals_do_not_copy_their_ast() {
    let parsed =
        parse_recovering("fun editing(value: Boolean) { if (value) {} else {}\nvalue. }").unwrap();
    let body = body(&parsed, "editing");
    assert!(body.conditionals.is_empty());
    assert!(body.operands.is_empty());
}
