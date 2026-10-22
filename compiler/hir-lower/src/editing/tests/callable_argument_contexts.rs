use super::receivers::{known_type, receiver};
use super::*;

#[test]
fn ambiguity_unknown_inputs_and_pending_parameters_do_not_supply_guessed_contexts() {
    for text in [
        "fun use(block: (String) -> Boolean) {}\nfun use(block: (Boolean) -> Boolean) {}\nfun editing(value: Boolean) { use({ value -> value. }) }",
        "fun <T> use(block: (T) -> Boolean) {}\nfun editing(value: Boolean) { use({ value -> value. }) }",
        "fun use(seed: String, block: (String) -> Boolean) {}\nfun editing(value: Boolean) { use(, { value -> value. }) }",
        "fun <T> use(block: (T) -> Boolean, seed: T) {}\nfun editing(value: Boolean) { use({ value -> value. }, \"later\") }",
        "fun use(first: (String) -> Boolean, second: (Boolean) -> Boolean) {}\nfun editing(value: String) { use({ it. }, { value -> value. }) }",
    ] {
        let mut analysis = analyze(text);
        let id = function(&analysis, "editing");
        let facts = analysis.receivers(id, || false).unwrap();
        let last = facts.last().unwrap();
        assert!(
            matches!(last.value_type, V::Unavailable(U::IncompleteSyntax)),
            "{text}: {last:?}"
        );
        assert!(last.local.is_none());
    }
}

#[test]
fn incompatible_parameter_and_suspend_headers_block_body_facts() {
    for expression in [
        "{ value: Boolean -> value. }",
        "{ first, value -> value. }",
        "{ value, value -> value. }",
        "suspend { value -> value. }",
        "fun(value: Boolean): Boolean { value. }",
        "fun(value: String): String { value. }",
    ] {
        let text = format!(
            "fun use(block: (String) -> Boolean) {{}}\nfun editing(value: Boolean) {{ use({expression}) }}"
        );
        let mut analysis = analyze(&text);
        assert!(
            matches!(
                receiver(&mut analysis).value_type,
                V::Unavailable(U::BodyFailure)
            ),
            "{text}"
        );
    }
    let mut analysis =
        analyze("fun use(block: Boolean) {}\nfun editing() { use({ value: String -> value. }) }");
    assert!(matches!(
        receiver(&mut analysis).value_type,
        V::Unavailable(U::IncompleteSyntax)
    ));
}

#[test]
fn only_named_lambda_calls_receive_an_implicit_return_label() {
    for expression in [
        "use({ value -> return@use value. })",
        "use(内部@ { value -> return@内部 value. })",
        "use(fun(value: String): Boolean { return value. })",
    ] {
        let text =
            format!("fun use(block: (String) -> Boolean) {{}}\nfun editing() = {expression}");
        let mut analysis = analyze(&text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
    }
    for expression in [
        "use({ value -> return value. })",
        "use(内部@ { value -> return@use value. })",
        "use(fun(value: String): Boolean { return@use value. })",
        "(use)({ value -> return@use value. })",
    ] {
        let text = format!("fun editing(use: ((String) -> Boolean) -> Boolean) {{ {expression} }}");
        let mut analysis = analyze(&text);
        assert!(
            matches!(
                receiver(&mut analysis).value_type,
                V::Unavailable(U::BodyFailure)
            ),
            "{text}"
        );
    }
}

#[test]
fn captures_and_caller_generics_keep_their_original_binding_identity() {
    for (declaration, expression, expected) in [
        (
            "fun use(block: () -> Boolean) {}",
            "use({ value. })",
            "Item",
        ),
        (
            "fun <T, R> use(seed: T, block: (T) -> R) {}",
            "use(value, { copy -> copy. })",
            "Item",
        ),
        (
            "fun use(block: () -> Boolean) {}",
            "use(fun() { value. })",
            "Item",
        ),
    ] {
        let text = format!("{declaration}\nfun <Item> editing(value: Item) {{ {expression} }}");
        let mut analysis = analyze(&text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), expected, "{text}");
        if expression.contains("value.") {
            assert_eq!(
                facts.local.as_ref().unwrap().definition.span.start as usize,
                text.find("value: Item").unwrap()
            );
        }
    }
    let text = "fun use(block: () -> Boolean) {}\nfun editing(value: Any) { if (value is String) { use({ value. }) } }";
    let mut analysis = analyze(text);
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "String");
    assert_eq!(
        facts.local.as_ref().unwrap().definition.span.start as usize,
        text.find("value: Any").unwrap()
    );
}

#[test]
fn entering_one_candidate_preserves_outer_scopes_for_sibling_branches() {
    let text = "fun use(block: (Boolean) -> Boolean) {}\nfun editing(value: Any) { if (value is String) { use({ value -> value. }) } else { value. } }";
    let mut analysis = analyze(text);
    let id = function(&analysis, "editing");
    let facts = analysis.receivers(id, || false).unwrap().to_vec();
    assert_eq!(facts.len(), 2);
    assert_eq!(known_type(&mut analysis, &facts[0]), "Boolean");
    assert_eq!(known_type(&mut analysis, &facts[1]), "Any");
    assert_eq!(
        facts[1].local.as_ref().unwrap().definition.span.start as usize,
        text.find("value: Any").unwrap()
    );
}

#[test]
fn cancellation_discards_candidate_contexts_before_a_clean_retry() {
    let text = "fun <T, R> use(seed: T, block: (T) -> R) {}\nfun editing() { use(\"known\", { value -> value. }) }";
    let mut analysis = analyze(text);
    let id = function(&analysis, "editing");
    let checks = std::cell::Cell::new(0);
    assert!(matches!(
        analysis.receivers(id, || {
            checks.set(checks.get() + 1);
            checks.get() == 10
        }),
        Err(EditingCancelled)
    ));
    assert!(!analysis.recovered_facts.contains_key(&id));
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "String");
}
