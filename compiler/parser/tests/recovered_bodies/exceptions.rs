use super::*;

#[test]
fn try_handlers_and_finally_keep_their_actual_sibling_blocks() {
    let text = "fun editing(value: String) { try { value. } catch (错误: Exception) { 错误. } finally { value. } }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    let [try_] = body.tries.as_slice() else {
        panic!("one exception region: {body:?}")
    };
    assert_eq!(try_.start as usize, text.find("try").unwrap());
    let ast::RecoveredTryBlock::Incomplete(first) = try_.body else {
        panic!("unfinished try block")
    };
    let ast::RecoveredCatch::Clause {
        name,
        body: ast::RecoveredTryBlock::Incomplete(handler),
        ..
    } = &try_.catches[0]
    else {
        panic!("known catch header")
    };
    assert_eq!(name.text, "错误");
    assert_eq!(source(text, name.span), "错误");
    let ids = [first, *handler, try_.finally_body.unwrap()];
    assert_eq!(body.selectors.len(), ids.len());
    for (selector, id) in body.selectors.iter().zip(ids) {
        assert_eq!(selector.block, Some(id));
        assert_eq!(
            body.blocks
                .iter()
                .find(|block| block.id == id)
                .unwrap()
                .parent,
            try_.block
        );
    }
    assert_eq!(parsed.syntax().text().to_string(), text);
}

#[test]
fn a_partial_handler_retains_the_complete_try_body_for_exception_flow() {
    let text =
        "fun editing(failure: Throwable) { try { throw failure } catch (error: Throwable) { error.";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    let ast::RecoveredTryBlock::Complete(complete) = &body.tries[0].body else {
        panic!("complete try input")
    };
    assert_eq!(source(text, complete.span), "{ throw failure }");
    assert!(matches!(
        body.tries[0].catches[0],
        ast::RecoveredCatch::Clause {
            body: ast::RecoveredTryBlock::Incomplete(_),
            ..
        }
    ));
    assert!(body.tries[0].finally_body.is_none());
}

#[test]
fn finally_queries_keep_complete_preceding_blocks_without_recovered_duplicates() {
    let text =
        "fun editing(value: String) { try {} catch (error: Throwable) {} finally { value. } }";
    let parsed = parse_recovering(text).unwrap();
    let body = body(&parsed, "editing");
    assert!(matches!(
        body.tries[0].body,
        ast::RecoveredTryBlock::Complete(_)
    ));
    assert!(matches!(
        body.tries[0].catches[0],
        ast::RecoveredCatch::Clause {
            body: ast::RecoveredTryBlock::Complete(_),
            ..
        }
    ));
    assert_eq!(body.blocks.len(), 2);
}

#[test]
fn eof_or_missing_handlers_do_not_discard_a_known_try_prefix() {
    for text in [
        "fun editing(value: String) { try { value.",
        "fun editing(value: String) { try { value. } }",
        "fun editing(value: String) { try { value. } catch (error: ) {} }",
    ] {
        let parsed = parse_recovering(text).unwrap();
        let body = body(&parsed, "editing");
        assert_eq!(body.tries.len(), 1, "{text}");
        assert!(matches!(
            body.tries[0].body,
            ast::RecoveredTryBlock::Incomplete(_)
        ));
        assert!(body.tries[0].catches.is_empty());
        assert_eq!(parsed.syntax().text().to_string(), text);
    }
}

#[test]
fn absent_opening_blocks_and_complete_tries_do_not_create_recovery_inputs() {
    for text in [
        "fun editing(value: Boolean) { try value. }",
        "fun editing(value: Boolean) { try {} finally {}; value. }",
        "fun editing(value: Boolean) { try {} catch (error: Throwable) {}; value. }",
    ] {
        let parsed = parse_recovering(text).unwrap();
        assert!(body(&parsed, "editing").tries.is_empty(), "{text}");
    }
}
