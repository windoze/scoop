use super::candidates::call_candidates;
use super::receivers::{known_type, receiver};
use super::*;

#[test]
fn for_patterns_bind_actual_iterator_element_types_and_sources() {
    for (text, expected, pattern) in [
        (
            "fun editing(source: Iterable<Boolean>) { for (item in source) { item. } }",
            "Boolean",
            "item in",
        ),
        (
            "fun editing(source: Iterable<String>) { for (item in source) item. }",
            "String",
            "item in",
        ),
        (
            "fun <T> editing(source: Iterable<T>) { for (item in source) { item. } }",
            "T",
            "item in",
        ),
        (
            "fun editing(source: Iterable<(Boolean, String)>) { for ((flag, text) in source) { text. } }",
            "String",
            "text) in",
        ),
        (
            "class Feed<T>(val values: Iterator<T>) { operator fun iterator(): Iterator<T> = values }\nfun editing(source: Feed<String>) { for (item in source) { item. } }",
            "String",
            "item in",
        ),
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), expected, "{text}");
        assert_eq!(
            facts.local.unwrap().definition.span.start as usize,
            text.find(pattern).unwrap()
        );
    }
}

#[test]
fn for_scopes_and_labels_follow_the_original_iteration_body() {
    for text in [
        "fun editing(source: Iterable<String>, item: Boolean) { cycle@ for (item in source) { if (false) continue@cycle; item. } }",
        "fun editing(source: Iterable<String>) { outer@ while (true) { for (item in source) { if (false) break@outer; item. } } }",
        "fun editing(source: Iterable<String>) { outer@ while (true) { for (item in if (true) source else continue@outer) { item. } } }",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
    }
    for text in [
        "fun editing(source: Iterable<String>) { for (item in source) { val item = true; item. } }",
        "fun editing(source: Iterable<Boolean>) { for (item in source) { item = true; item. } }",
        "fun editing(source: Iterable<Boolean>) { for (item in source) { break@missing; item. } }",
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
fn iteration_source_uses_outer_flow_before_unknown_backedge_facts_are_removed() {
    for (text, expected) in [
        (
            "fun editing(source: Any) { if (source is Iterable<Boolean>) { for (item in source) { item. } } }",
            "Boolean",
        ),
        (
            "fun editing(source: Iterable<Boolean>, value: Any) { var local: Any = value; if (local is String) { for (item in source) { local. } } }",
            "Any",
        ),
        (
            "fun editing(source: Iterable<Boolean>, value: Any) { if (value is String) { for (item in source) { value. } } }",
            "String",
        ),
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), expected, "{text}");
    }
}

#[test]
fn failed_iteration_protocols_or_patterns_do_not_publish_bindings() {
    for text in [
        "fun editing(source: Boolean) { for (item in source) { source. } }",
        "class Wrong { operator fun iterator(): Boolean = true }\nfun editing(source: Wrong) { for (item in source) { source. } }",
        "fun editing(source: Iterable<Boolean>) { for (true in source) { source. } }",
        "fun editing(source: Iterable<(Boolean, String)>) { for ((item, item) in source) { source. } }",
        "fun editing(source: Iterable<Boolean>) { loop@ for (item in if (true) source else continue@loop) { source. } }",
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
fn unfinished_iteration_inputs_and_calls_preserve_real_binding_types() {
    let mut analysis = analyze("fun editing(source: Boolean) { for (item in source.) }");
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "Boolean");
    let candidates = call_candidates(
        "fun choose(value: Boolean, next: Boolean) {}\nfun choose(value: String, next: Boolean) {}\nfun editing(source: Iterable<String>) { for (item in source) { choose(item,",
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].parameters[0].type_label, "String");
}

#[test]
fn cancelling_iteration_recovery_does_not_publish_a_pattern_scope() {
    let mut analysis =
        analyze("fun editing(source: Iterable<String>) { for (item in source) { item. } }");
    let function = function(&analysis, "editing");
    let checks = std::cell::Cell::new(0);
    assert!(matches!(
        analysis.receivers(function, || {
            checks.set(checks.get() + 1);
            checks.get() == 6
        }),
        Err(EditingCancelled)
    ));
    assert!(!analysis.recovered_facts.contains_key(&function));
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "String");
}
