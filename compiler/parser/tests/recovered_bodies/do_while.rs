use super::*;

#[test]
fn an_unfinished_do_body_does_not_publish_a_condition_context() {
    let text = "fun editing(value: Any) { 重复@ do { value. } while (value.) }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    let [do_while] = body.do_while_loops.as_slice() else {
        panic!("one recovered do loop: {body:?}");
    };
    assert_eq!(do_while.label.as_ref().unwrap().text, "重复");
    assert_eq!(do_while.start as usize, text.find("重复").unwrap());
    let ast::RecoveredDoWhilePart::Body(ast::RecoveredControlBody::Block(id)) = do_while.part
    else {
        panic!("the unfinished body keeps its real block");
    };
    assert_eq!(body.selectors.len(), 2);
    assert_eq!(body.selectors[0].block, Some(id));
    assert_eq!(body.selectors[1].block, do_while.block);
    assert_eq!(parsed.syntax().text().to_string(), text);
}

#[test]
fn a_do_condition_keeps_the_original_complete_body_and_expression_entry() {
    let text = "fun editing() { do { val value = true } while (value.) }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    let ast::RecoveredDoWhilePart::Condition {
        body: complete,
        expression_start,
    } = &body.do_while_loops[0].part
    else {
        panic!("the complete body supplies the condition scope");
    };
    assert_eq!(source(text, complete.span), "{ val value = true }");
    assert_eq!(complete.statements.len(), 1);
    assert_eq!(*expression_start as usize, text.rfind("value.").unwrap());
    assert_eq!(body.blocks.len(), 1);
}

#[test]
fn single_statement_do_bodies_and_eof_keep_their_actual_entries() {
    let text = "fun editing(value: Boolean) { cycle@ do value.";
    let parsed = parse_recovering(text).unwrap();
    let unfinished = body(&parsed, "editing");
    assert_eq!(unfinished.blocks.len(), 1);
    assert_eq!(
        unfinished.do_while_loops[0].part,
        ast::RecoveredDoWhilePart::Body(ast::RecoveredControlBody::Statement(
            text.rfind("value.").unwrap() as u32
        ))
    );
    let text = "fun editing() { do val value = true while (value.) }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    let ast::RecoveredDoWhilePart::Condition { body: complete, .. } = &body.do_while_loops[0].part
    else {
        panic!("a complete single statement remains the original control body");
    };
    assert_eq!(source(text, complete.span), "val value = true");
}

#[test]
fn complete_do_loops_do_not_copy_their_bodies() {
    let parsed = parse_recovering(
        "fun editing(value: Boolean) { do { val local = value } while (local); value. }",
    )
    .unwrap();
    assert!(body(&parsed, "editing").do_while_loops.is_empty());
}
