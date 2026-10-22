use super::receivers::{known_type, receiver};
use super::*;

#[test]
fn trailing_literals_use_actual_parameters_with_or_without_parentheses() {
    for expression in [
        "use { value -> value. }",
        "use() { value -> value. }",
        "use { it. }",
        "use { value -> return@use value. }",
        "use 内部@ { value -> return@内部 value. }",
    ] {
        let text = format!(
            "fun use(block: (String) -> Boolean) {{}}\nfun editing(value: Boolean) {{ {expression} }}"
        );
        let mut analysis = analyze(&text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
    }
}

#[test]
fn ordinary_inference_defaults_and_named_mapping_supply_trailing_contexts() {
    for (declaration, expression) in [
        (
            "fun <T, R> use(seed: T, block: (T) -> R) {}",
            "use(\"known\") { value -> value. }",
        ),
        (
            "fun <T, R> use(seed: T, block: (T) -> R) {}",
            "use(seed = \"known\") { value -> value. }",
        ),
        (
            "fun <T, R> use(block: (T) -> R) {}",
            "use<String, _> { value -> value. }",
        ),
        (
            "fun use(flag: Boolean = missing, block: (String) -> Boolean) {}",
            "use { value -> value. }",
        ),
    ] {
        let text = format!("{declaration}\nfun editing() {{ {expression} }}");
        let mut analysis = analyze(&text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
    }
}

#[test]
fn members_extensions_safe_calls_and_constructors_use_the_same_argument_path() {
    for text in [
        "class Host { fun use(block: (String) -> Boolean) {} }\nfun editing(host: Host) { host.use { value -> value. } }",
        "class Host { fun use(block: (String) -> Boolean) {} }\nfun editing(host: Host?) { host?.use { value -> value. } }",
        "fun Boolean.use(block: (String) -> Boolean) {}\nfun editing() { true.use { value -> value. } }",
        "struct Host<T>(val seed: T, val block: (T) -> Boolean)\nfun editing() { Host(\"known\") { value -> value. } }",
        "fun <T, R> use(seed: T, block: T.() -> R) {}\nfun editing() { use(\"known\") { this. } }",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
    }
}

#[test]
fn grouped_callable_results_keep_their_invoke_target_and_no_implicit_label() {
    for text in [
        "fun editing(action: ((String) -> Boolean) -> Boolean) { (action) { value -> value. } }",
        "fun factory(): ((String) -> Boolean) -> Boolean = { true }\nfun editing() { (factory()) { value -> value. } }",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
        let id = function(&analysis, "editing");
        let calls = analysis.incomplete_calls(id, || false).unwrap();
        assert_eq!(calls.len(), 1);
        assert!(calls[0].name.is_none());
        assert!(calls[0].arguments.is_none());
    }
    let mut analysis = analyze(
        "fun editing(action: ((String) -> Boolean) -> Boolean) { (action) { value -> return@action value. } }",
    );
    assert!(matches!(
        receiver(&mut analysis).value_type,
        V::Unavailable(U::BodyFailure)
    ));
}

#[test]
fn nested_trailing_literals_keep_inferred_inputs_and_capture_definitions() {
    for (expression, name) in [
        (
            "use { outer -> map(outer) { inner -> inner. } }",
            "inner ->",
        ),
        ("use { outer -> map(true) { inner -> outer. } }", "outer ->"),
    ] {
        let text = format!(
            "fun use(block: (String) -> Boolean) {{}}\nfun <T, R> map(seed: T, block: (T) -> R) {{}}\nfun editing() {{ {expression} }}"
        );
        let mut analysis = analyze(&text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String");
        assert_eq!(
            facts.local.as_ref().unwrap().definition.span.start as usize,
            text.find(name).unwrap()
        );
    }
}

#[test]
fn pending_candidates_and_unknown_prefix_arguments_do_not_reach_a_trailing_body() {
    for text in [
        "fun use(block: (String) -> Boolean) {}\nfun use(block: (Boolean) -> Boolean) {}\nfun editing(value: String) { use { value -> value. } }",
        "fun <T> use(block: (T) -> Boolean) {}\nfun editing(value: String) { use { value -> value. } }",
        "fun use(seed: Boolean, block: (String) -> Boolean) {}\nfun editing(value: String) { use(,) { value -> value. } }",
    ] {
        let mut analysis = analyze(text);
        assert!(
            matches!(
                receiver(&mut analysis).value_type,
                V::Unavailable(U::IncompleteSyntax)
            ),
            "{text}"
        );
    }
}

#[test]
fn suspend_and_return_rules_still_apply_to_trailing_literals() {
    let mut analysis = analyze(
        "fun use(block: suspend (String) -> Boolean) {}\nfun editing() { use suspend { value -> value. } }",
    );
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "String");
    for expression in [
        "use suspend { value -> value. }",
        "use { value -> return value. }",
        "use 内部@ { value -> return@use value. }",
    ] {
        let text =
            format!("fun use(block: (String) -> Boolean) {{}}\nfun editing() {{ {expression} }}");
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
