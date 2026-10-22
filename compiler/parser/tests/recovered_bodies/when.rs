use super::*;

#[test]
fn recovered_when_keeps_ordered_heads_and_actual_body_entries() {
    let text = "fun editing(source: Any) { when (val subject = source) { is String -> { subject. }\nelse -> subject. } }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    let [when] = body.whens.as_slice() else {
        panic!("one recovered when: {body:?}")
    };
    assert_eq!(when.start as usize, text.find("when").unwrap());
    assert!(matches!(when.subject, ast::WhenSubject::Declaration(_)));
    assert_eq!(when.arms.len(), 2);
    let ast::RecoveredWhenArm::Head {
        body: Some(ast::RecoveredControlBody::Block(id)),
        ..
    } = &when.arms[0]
    else {
        panic!("real first block")
    };
    assert_eq!(body.selectors[0].block, Some(*id));
    let ast::RecoveredWhenArm::Head {
        condition: ast::WhenArmCondition::Else,
        body: Some(ast::RecoveredControlBody::Statement(start)),
        ..
    } = &when.arms[1]
    else {
        panic!("real fallback statement")
    };
    assert_eq!(*start as usize, text.rfind("subject.").unwrap());
    assert_eq!(body.selectors[1].block, when.block);
    assert_eq!(parsed.syntax().text().to_string(), text);
}

#[test]
fn previous_complete_heads_and_eof_are_retained_without_copying_complete_bodies() {
    let text = "fun editing(value: Boolean) { when (value) { false -> {}\ntrue -> { value.";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    assert_eq!(body.whens[0].arms.len(), 2);
    assert!(matches!(
        body.whens[0].arms[0],
        ast::RecoveredWhenArm::Head { body: None, .. }
    ));
    assert!(matches!(
        body.whens[0].arms[1],
        ast::RecoveredWhenArm::Head {
            body: Some(ast::RecoveredControlBody::Block(_)),
            ..
        }
    ));
    assert!(body.blocks.iter().all(|block| !block.closed));
}

#[test]
fn an_incomplete_guard_retains_its_real_pattern_and_expression_start() {
    let text = "fun editing(value: (Boolean, String)) { when (value) { case (flag, 文本) if 文本.";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    let ast::RecoveredWhenArm::Guard {
        condition: ast::WhenArmCondition::Case(ast::Pattern::Tuple { .. }),
        expression_start,
        ..
    } = &body.whens[0].arms[0]
    else {
        panic!("complete pattern before the guard")
    };
    assert_eq!(*expression_start as usize, text.rfind("文本.").unwrap());
    assert_eq!(parsed.syntax().text().to_string(), text);
}

#[test]
fn partial_conditions_retain_only_the_previous_complete_alternatives() {
    for (text, count, marker) in [
        (
            "fun editing(value: Any) { when { value !is String, value.",
            1,
            "value.",
        ),
        (
            "fun editing(value: Any, source: String) { when (value) { in source.",
            0,
            "source.",
        ),
    ] {
        let parsed = parse_recovering(text).unwrap();
        let body = body(&parsed, "editing");
        let ast::RecoveredWhenArm::Condition {
            previous,
            expression_start,
            ..
        } = &body.whens[0].arms[0]
        else {
            panic!("an unfinished ordinary condition: {text}")
        };
        assert_eq!(previous.len(), count);
        assert_eq!(*expression_start as usize, text.rfind(marker).unwrap());
    }
}

#[test]
fn unfinished_subjects_use_the_original_operand_path() {
    for text in [
        "fun editing(source: String) { when (source.",
        "fun editing(source: String) { when (val value: String = source.",
    ] {
        let parsed = parse_recovering(text).unwrap();
        let body = body(&parsed, "editing");
        assert!(body.whens.is_empty());
        assert!(
            body.operands
                .iter()
                .any(
                    |operand| operand.statement_start as usize == text.find("when").unwrap()
                        && matches!(operand.context, ast::RecoveredOperandContext::WhenSubject)
                )
        );
        assert_eq!(body.selectors.len(), 1);
    }
}

#[test]
fn unknown_heads_and_arms_after_else_are_not_projected_as_usable_headers() {
    for text in [
        "fun editing(value: String) { when (value) { _ -> {}\nelse -> value. } }",
        "fun editing(value: String) { when (value) { else -> {}\nis String -> value. } }",
    ] {
        let parsed = parse_recovering(text).unwrap();
        let body = body(&parsed, "editing");
        assert!(
            body.whens[0]
                .arms
                .iter()
                .any(|arm| matches!(arm, ast::RecoveredWhenArm::Unavailable(_))),
            "{text}"
        );
        assert!(parsed.ast().is_none());
    }
}

#[test]
fn complete_when_productions_do_not_copy_their_subjects_or_heads() {
    let parsed = parse_recovering(
        "fun editing(value: Boolean) { when (value) { true -> {}\nelse -> {} }; value. }",
    )
    .unwrap();
    assert!(body(&parsed, "editing").whens.is_empty());
}
