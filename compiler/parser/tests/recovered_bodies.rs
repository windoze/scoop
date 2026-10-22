use scoop_ast::{self as ast, DeclarationFragment, RecoveredReceiver, RecoveredStatement};
use scoop_parser::{ParsedFile, parse_recovering};

#[path = "recovered_bodies/conditionals.rs"]
mod conditionals;

#[path = "recovered_bodies/loops.rs"]
mod loops;

#[path = "recovered_bodies/iteration.rs"]
mod iteration;

fn body<'a>(parsed: &'a ParsedFile, name: &str) -> &'a ast::RecoveredFunctionBody {
    parsed
        .declaration_data()
        .declarations
        .iter()
        .find_map(|item| {
            let DeclarationFragment::FunctionBody(body) = &item.value else {
                return None;
            };
            let declaration = &parsed.declarations()[item.declaration.0];
            (declaration.name.as_ref().unwrap().text == name).then_some(body.as_ref())
        })
        .unwrap_or_else(|| panic!("missing body of {name}: {:?}", parsed.diagnostics()))
}

fn source(text: &str, span: ast::Span) -> &str {
    &text[span.start as usize..span.end as usize]
}

#[test]
fn independent_statements_surround_a_real_unknown_operation() {
    let text = "fun editing(user: User) { val before = user\nuser.\nval after = before }";
    let parsed = parse_recovering(text).unwrap();
    assert!(parsed.ast().is_none());
    let body = body(&parsed, "editing");
    assert_eq!(body.blocks.len(), 1);
    let block = &body.blocks[0];
    assert_eq!(body.root, ast::RecoveredBodyRoot::Block(block.id));
    assert!(block.closed);
    assert!(block.parent.is_none());
    assert!(matches!(
        block.statements.as_slice(),
        [
            RecoveredStatement::Complete(_),
            RecoveredStatement::Unavailable(_),
            RecoveredStatement::Complete(_)
        ]
    ));
    let selector = &body.selectors[0];
    assert_eq!(selector.block, Some(block.id));
    assert_eq!(source(text, selector.dot), ".");
    let RecoveredReceiver::Complete(receiver) = &selector.receiver else {
        panic!("complete receiver");
    };
    assert_eq!(source(text, receiver.span()), "user");
}

#[test]
fn eof_preserves_the_actual_prefix_without_a_successful_block() {
    let text = "fun editing(user: User) { val before = user\nuser. \n";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    let block = &body.blocks[0];
    assert!(!block.closed);
    assert!(matches!(
        block.statements.as_slice(),
        [
            RecoveredStatement::Complete(_),
            RecoveredStatement::Unavailable(_)
        ]
    ));
    assert_eq!(block.span.end as usize, text.len());
    assert_eq!(
        body.selectors[0].selector,
        ast::Span::new(text.len() as u32, text.len() as u32)
    );
}

#[test]
fn failed_calls_and_literals_do_not_publish_a_complete_receiver() {
    for (text, reaches_selector) in [
        ("fun editing() { invoke(, value). }", true),
        ("fun editing() { \"\\q\". }", false),
    ] {
        let parsed = parse_recovering(text).unwrap();
        let body = body(&parsed, "editing");
        if reaches_selector {
            assert_eq!(body.selectors.len(), 1);
            assert!(matches!(
                body.selectors[0].receiver,
                RecoveredReceiver::Unavailable(_)
            ));
        } else {
            assert!(body.selectors.is_empty(), "{text}");
        }
        assert!(matches!(
            body.blocks[0].statements[0],
            RecoveredStatement::Unavailable(_)
        ));
    }
}

#[test]
fn an_incomplete_nested_body_is_not_a_complete_outer_receiver() {
    let text = "fun editing() { provide { value. }. }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    assert_eq!(body.selectors.len(), 2);
    assert!(matches!(
        body.selectors[0].receiver,
        RecoveredReceiver::Complete(_)
    ));
    let RecoveredReceiver::Unavailable(span) = body.selectors[1].receiver else {
        panic!("the outer receiver contains an incomplete lambda body");
    };
    assert_eq!(source(text, span), "provide { value. }");
}

#[test]
fn nested_blocks_keep_grammar_parents_and_do_not_complete_the_control_statement() {
    let text = "fun editing(user: User) { if (true) { user. }\nval after = user }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    assert_eq!(body.blocks.len(), 2);
    let root = body
        .blocks
        .iter()
        .find(|block| block.parent.is_none())
        .unwrap();
    let nested = body
        .blocks
        .iter()
        .find(|block| block.parent.is_some())
        .unwrap();
    assert_eq!(nested.parent, Some(root.id));
    assert_eq!(body.selectors[0].block, Some(nested.id));
    assert!(matches!(
        root.statements.as_slice(),
        [
            RecoveredStatement::Unavailable(_),
            RecoveredStatement::Complete(_)
        ]
    ));
}

#[test]
fn nested_functions_own_their_recovery_and_start_a_separate_body_scope() {
    let text = "fun outer() { fun inner(value: User) { value. }\nouter. }";
    let parsed = parse_recovering(text).unwrap();
    let outer = body(&parsed, "outer");
    let inner = body(&parsed, "inner");
    assert_eq!(outer.selectors.len(), 1);
    assert_eq!(inner.selectors.len(), 1);
    assert!(inner.blocks[0].parent.is_none());
    let RecoveredReceiver::Complete(receiver) = &inner.selectors[0].receiver else {
        panic!("known receiver");
    };
    assert_eq!(source(text, receiver.span()), "value");
}

#[test]
fn unicode_safe_navigation_and_expression_bodies_keep_exact_positions() {
    for text in [
        "fun 中文(值: User) {\r\n值?.\r\n}",
        "fun 中文(值: User) = 值?.",
    ] {
        let parsed = parse_recovering(text).unwrap();
        let body = body(&parsed, "中文");
        let selector = &body.selectors[0];
        assert_eq!(selector.navigation, ast::Navigation::Safe);
        assert_eq!(source(text, selector.dot), "?.");
        let RecoveredReceiver::Complete(receiver) = &selector.receiver else {
            panic!("known receiver");
        };
        assert_eq!(source(text, receiver.span()), "值");
        assert_eq!(selector.block.is_none(), text.contains(" = "));
        if text.contains(" = ") {
            assert_eq!(
                body.root,
                ast::RecoveredBodyRoot::Expression(receiver.span().start)
            );
        }
        assert_eq!(parsed.syntax().text().to_string(), text);
    }
}

#[test]
fn complete_blocks_do_not_copy_statements_into_recovery_data() {
    let parsed =
        parse_recovering("fun healthy(user: User) { val copy = user\nconsume(copy) }").unwrap();
    assert!(parsed.ast().is_some());
    assert!(
        !parsed
            .declaration_data()
            .declarations
            .iter()
            .any(|item| matches!(item.value, DeclarationFragment::FunctionBody(_)))
    );
}

#[test]
fn callable_literals_keep_their_own_parameter_scope_boundaries() {
    let text = "fun editing(value: User) { consume { value -> value. }\nval other = fun (value: User) { value. } }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    let root = body
        .blocks
        .iter()
        .find(|block| block.parent.is_none())
        .unwrap();
    assert_eq!(root.scope, ast::RecoveredBlockScope::Block);
    for scope in [
        ast::RecoveredBlockScope::Lambda,
        ast::RecoveredBlockScope::AnonymousFunction,
    ] {
        let block = body
            .blocks
            .iter()
            .find(|block| block.scope == scope)
            .unwrap();
        assert_eq!(block.parent, Some(root.id));
        assert!(
            body.selectors
                .iter()
                .any(|selector| selector.block == Some(block.id))
        );
    }
}

#[test]
fn unfinished_operands_retain_real_statement_entries_and_complete_headers() {
    for (text, keyword) in [
        (
            "fun editing(value: User) { val copy: User = value. }",
            "val",
        ),
        (
            "fun editing(value: User) { return@editing value. }",
            "return",
        ),
        ("fun editing(value: User) { throw value. }", "throw"),
    ] {
        let parsed = parse_recovering(text).unwrap();
        let body = body(&parsed, "editing");
        assert_eq!(body.operands.len(), 1);
        let operand = &body.operands[0];
        assert_eq!(operand.block, body.selectors[0].block);
        assert!(text[operand.statement_start as usize..].starts_with(keyword));
        assert!(text[operand.expression_start as usize..].starts_with("value."));
        match &operand.context {
            ast::RecoveredOperandContext::Initializer { annotation } => {
                assert_eq!(source(text, annotation.as_ref().unwrap().span), "User")
            }
            ast::RecoveredOperandContext::Return { label } => {
                assert_eq!(label.as_ref().unwrap().text, "editing")
            }
            ast::RecoveredOperandContext::Throw => assert_eq!(keyword, "throw"),
            ast::RecoveredOperandContext::IfCondition
            | ast::RecoveredOperandContext::WhileCondition { .. }
            | ast::RecoveredOperandContext::ForIterable => panic!("not a control operand"),
        }
        assert_eq!(parsed.syntax().text().to_string(), text);
    }
}

#[test]
fn completed_operands_and_bad_headers_do_not_add_recovery_entries() {
    for text in [
        "fun editing() { val done = call(); broken. }",
        "fun editing() { val item: Box<> = call( }",
        "fun editing() { return@ = call( }",
    ] {
        let parsed = parse_recovering(text).unwrap();
        assert!(body(&parsed, "editing").operands.is_empty(), "{text}");
    }
}
