use super::candidates::call_candidates;
use super::receivers::{known_type, receiver};
use super::*;

#[test]
fn explicit_parameters_shadow_outer_names_and_keep_their_definition_sites() {
    for expression in [
        "{ value: String -> value. }",
        "fun(value: String): Boolean { value. }",
    ] {
        let text = format!("fun editing(value: Boolean) {{ val action = {expression} }}");
        let mut analysis = analyze(&text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String");
        assert_eq!(
            facts.local.as_ref().unwrap().definition.span.start as usize,
            text.find("value: String").unwrap()
        );
    }
}

#[test]
fn initializer_and_return_expectations_supply_real_lambda_parameters() {
    for text in [
        "fun editing(value: Boolean) { val action: (String) -> Boolean = { value -> value. } }",
        "fun editing(it: Boolean) { val action: (String) -> Boolean = { it. } }",
        "fun editing(): (String) -> Boolean { return { value -> value. } }",
        "fun editing(): (String) -> Boolean = { value -> value. }",
        "fun editing() { val outer = fun(): (String) -> Boolean { return { value -> value. } } }",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
    }
}

#[test]
fn lambda_destructuring_uses_the_original_irrefutable_pattern_rules() {
    for text in [
        "fun editing() { val action = { (value, flag): (String, Boolean), extra: Boolean -> value. } }",
        "fun editing() { val action: ((String, Boolean)) -> Boolean = { (value, flag) -> value. } }",
        "struct Box<T>(val value: T)\nfun editing() { val action = { Box(value): Box<String> -> value. } }",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
    }
    for text in [
        "fun editing() { val action = { (value, value): (String, Boolean), extra: Boolean -> value. } }",
        "fun editing() { val action = { Some(value): String? -> value. } }",
    ] {
        let mut analysis = analyze(text);
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
fn captures_preserve_their_original_binding_and_generic_type() {
    for expression in ["{ value. }", "fun() { value. }"] {
        let text = format!("fun <Item> editing(value: Item) {{ val action = {expression} }}");
        let mut analysis = analyze(&text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "Item");
        assert_eq!(
            facts.local.as_ref().unwrap().definition.span.start as usize,
            text.find("value: Item").unwrap()
        );
    }
    let text = "fun editing(value: Any) { if (value is String) { val action = { value. } } }";
    let mut analysis = analyze(text);
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "String");
    assert_eq!(
        facts.local.unwrap().definition.span.start as usize,
        text.find("value: Any").unwrap()
    );
}

#[test]
fn lambda_and_anonymous_receivers_use_their_declared_type_and_outer_receivers() {
    for (text, expected) in [
        (
            "fun editing() { val action: String.() -> Boolean = { this. } }",
            "String",
        ),
        (
            "fun editing() { val action = fun String.(): Boolean { this. } }",
            "String",
        ),
        (
            "class Host(val value: String) { fun editing() { val action = { this. } } }",
            "Host",
        ),
        (
            "class Host(val value: String) { fun editing() { val action = fun() { value. } } }",
            "String",
        ),
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), expected, "{text}");
    }
}

#[test]
fn unresolved_and_incompatible_headers_do_not_borrow_outer_parameters() {
    for expression in [
        "{ value -> value. }",
        "fun(value: Missing) { value. }",
        "fun(value: String, value: Boolean) { value. }",
        "{ value: String, value: Boolean -> value. }",
    ] {
        let text = format!("fun editing(value: Boolean) {{ val action = {expression} }}");
        let mut analysis = analyze(&text);
        assert!(
            matches!(
                receiver(&mut analysis).value_type,
                V::Unavailable(U::BodyFailure)
            ),
            "{text}"
        );
    }
    for expression in [
        "{ value: Boolean -> value. }",
        "{ first, value -> value. }",
        "fun(value: Boolean): Boolean { value. }",
        "suspend { value -> value. }",
    ] {
        let text = format!(
            "fun editing(value: Boolean) {{ val action: (String) -> Boolean = {expression} }}"
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
}

#[test]
fn nested_callable_prefixes_provide_actual_partial_call_arguments() {
    for expression in [
        "{ value: String -> choose(value,",
        "fun(value: String) { choose(value,",
        "fun(value: String) { val action = { choose(value,",
    ] {
        let candidates = call_candidates(&format!(
            "fun choose(value: String, next: Boolean): Boolean = next\nfun choose(value: Boolean, next: Boolean): Boolean = next\nfun editing() {{ val action = {expression}"
        ));
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].parameters[0].type_label, "String");
    }
}
