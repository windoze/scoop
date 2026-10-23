use super::receivers::{known_type, receiver};
use super::*;

#[test]
fn labels_resolve_to_the_current_callable_even_in_an_expression_body() {
    for expression in [
        "内部@ { value: String -> return@内部 value. }",
        "内部@ fun(value: String): Boolean { return@内部 value. }",
        "fun(value: String): Boolean { return value. }",
    ] {
        let text = format!("fun editing() = {expression}");
        let mut analysis = analyze(&text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String");
    }
    for expression in [
        "{ value: String -> return value. }",
        "内部@ { value: String -> return@editing value. }",
        "fun(value: String): Boolean { return@editing value. }",
    ] {
        let text = format!("fun editing() {{ val action = {expression} }}");
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
fn outer_loop_targets_do_not_cross_a_callable_boundary() {
    for expression in [
        "{ break@outer; value. }",
        "fun() { continue@outer; value. }",
    ] {
        let text = format!(
            "fun editing(value: String) {{ outer@ while (true) {{ val action = {expression} }} }}"
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
    let mut analysis = analyze(
        "fun editing(value: String) { outer@ while (true) { val action = { inner@ while (true) { break@inner }; value. } } }",
    );
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "String");
}

#[test]
fn suspension_is_checked_inside_the_literal_instead_of_its_enclosing_function() {
    let declaration = "suspend fun load(): String = \"ready\"";
    for expression in [
        "suspend { val value = load(); value. }",
        "suspend fun() { val value = load(); value. }",
    ] {
        let mut analysis = analyze(&format!(
            "{declaration}\nfun editing() {{ val action = {expression} }}"
        ));
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String");
    }
    for expression in [
        "{ val value = load(); value. }",
        "fun() { val value = load(); value. }",
    ] {
        let mut analysis = analyze(&format!(
            "{declaration}\nsuspend fun editing() {{ val action = {expression} }}"
        ));
        assert!(matches!(
            receiver(&mut analysis).value_type,
            V::Unavailable(U::BodyFailure)
        ));
    }
}

#[test]
fn querying_one_literal_restores_outer_bindings_and_flow_for_sibling_branches() {
    let text = "fun editing(value: Any) { if (value is String) { val action = { value: Boolean -> value. } } else { value. } }";
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
fn cancellation_discards_nested_literal_scopes_and_retries_cleanly() {
    let text = "fun editing(value: String) { val outer = fun() { val inner = { value. } } }";
    let mut analysis = analyze(text);
    let id = function(&analysis, "editing");
    let checks = std::cell::Cell::new(0);
    assert!(matches!(
        analysis.receivers(id, || {
            checks.set(checks.get() + 1);
            checks.get() == 11
        }),
        Err(EditingCancelled)
    ));
    assert!(!analysis.recovered_facts.contains_key(&id));
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "String");
    assert_eq!(
        facts.local.as_ref().unwrap().definition.span.start as usize,
        text.find("value: String").unwrap()
    );
}
