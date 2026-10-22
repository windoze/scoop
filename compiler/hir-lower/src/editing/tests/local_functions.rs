use super::receivers::{known_type, receiver};
use super::*;

#[test]
fn local_parameters_use_their_own_header_and_definition() {
    for declaration in [
        "fun local(value: String) { value. }",
        "fun local(value: String) = value.",
        "fun local(value: String): Boolean { return value. }",
        "fun local(value: String): Boolean { return@local value. }",
    ] {
        let text = format!("fun editing(value: Boolean) {{ {declaration} }}");
        let mut analysis = analyze(&text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
        assert_eq!(
            facts.local.as_ref().unwrap().definition.span.start as usize,
            text.find("value: String").unwrap()
        );
    }
}

#[test]
fn local_generic_parameters_keep_their_actual_source_context_after_scope_exit() {
    for (declaration, expected) in [
        ("fun <Item> local(value: Item) { value. }", "Item"),
        ("fun <Item> local(value: Box<Item>) { value. }", "Box<Item>"),
        (
            "fun <Item> local(value: Item) { val action = { value. } }",
            "Item",
        ),
        (
            "fun <Item> local(value: Item) { fun nested() { value. } }",
            "Item",
        ),
    ] {
        let text = format!(
            "struct Box<T>(val value: T)\nfun <Outer> editing(outer: Outer) {{ {declaration} }}"
        );
        let mut analysis = analyze(&text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), expected, "{text}");
    }
}

#[test]
fn local_captures_keep_outer_bindings_inside_functions_and_literals() {
    for text in [
        "fun editing(value: String) { fun local() { value. } }",
        "fun editing(value: String) { fun first() { fun second() { value. } } }",
        "fun editing() { val action = { value: String -> fun local() { value. } } }",
        "fun use(block: (String) -> Boolean) {}\nfun editing() { use { value -> fun local() { value. } } }",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
        let definition = text
            .find("value: String")
            .or_else(|| text.find("value ->"))
            .unwrap();
        assert_eq!(
            facts.local.as_ref().unwrap().definition.span.start as usize,
            definition,
            "{text}"
        );
    }
}

#[test]
fn partial_self_calls_use_declared_generic_parameters_defaults_and_pending_results() {
    let text =
        "fun editing() { fun <Item> local(value: Item, flag: Boolean = true) = local(value, }";
    let mut analysis = analyze(text);
    let id = function(&analysis, "editing");
    let calls = analysis.incomplete_calls(id, || false).unwrap();
    assert_eq!(calls.len(), 1, "{calls:?}");
    let V::Known(candidates) = &calls[0].candidates else {
        panic!("self signature: {calls:?}")
    };
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].parameters[0].type_label, "Item");
    assert!(candidates[0].parameters[1].has_default);
    assert!(matches!(
        candidates[0].result,
        V::Unavailable(U::PendingInference)
    ));
}

#[test]
fn contexts_and_enclosing_receivers_use_the_original_local_function_scope() {
    for (text, expected) in [
        (
            "fun editing() { context(value: String) fun local() { value. } }",
            "String",
        ),
        (
            "class Host(val value: String) { fun editing() { fun local() { this. } } }",
            "Host",
        ),
        (
            "class Host(val value: String) { fun editing() { fun local() { value. } } }",
            "String",
        ),
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), expected, "{text}");
    }
}

#[test]
fn local_scope_exit_restores_outer_flow_and_generic_context_in_other_branches() {
    let text = "fun <Outer> editing(value: Outer) { if (true) { fun <Inner> local(value: Inner) { value. } } else { value. } }";
    let mut analysis = analyze(text);
    let id = function(&analysis, "editing");
    let facts = analysis.receivers(id, || false).unwrap().to_vec();
    assert_eq!(facts.len(), 2);
    for receiver in &facts {
        let local = receiver.local.as_ref().unwrap();
        let expected = if local.definition.span.start as usize == text.find("value: Inner").unwrap()
        {
            "Inner"
        } else {
            "Outer"
        };
        assert_eq!(known_type(&mut analysis, receiver), expected);
    }
}
