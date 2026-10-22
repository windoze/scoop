use super::candidates::call_candidates;
use super::receivers::{known_type, receiver};
use super::*;

#[test]
fn partial_when_subjects_use_the_outer_scope_without_the_new_binding() {
    for text in [
        "fun editing(value: String) { when (value.",
        "fun editing(value: String) { when (val value = value.",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
        assert_eq!(
            facts.local.unwrap().definition.span.start as usize,
            text.find("value").unwrap()
        );
    }
    let mut analysis = analyze("fun editing(value: String) { when (val local: Missing = value.");
    assert!(matches!(
        receiver(&mut analysis).value_type,
        V::Unavailable(U::BodyFailure)
    ));
}

#[test]
fn partial_when_guards_use_completed_primary_facts_and_bindings() {
    for text in [
        "fun editing(value: Any) { when (value) { is String if value.",
        "fun editing(source: (Boolean, String)) { when (source) { case (flag, value) if value.",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
    }
}

#[test]
fn partial_when_conditions_use_previous_false_paths_and_the_collection_entry() {
    for text in [
        "fun editing(value: Any) { when { value !is String, value.",
        "fun editing(value: Any) { when { value !is String -> {}\nvalue.",
        "fun editing(value: Any, source: String) { when (value) { in source.",
        "fun editing(value: Any) { when (value) { !is String, value.",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
    }
}

#[test]
fn unknown_when_headers_do_not_allow_later_arms_to_reuse_outer_facts() {
    for text in [
        "fun editing(value: String) { when (value) { _ -> {}\nelse -> value. } }",
        "fun editing(value: String) { when (value) { else -> {}\nis String -> value. } }",
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
    let mut analysis = analyze(
        "fun editing(value: String) { when (value) { is String if value.\nelse -> value. } }",
    );
    let id = function(&analysis, "editing");
    let facts = analysis.receivers(id, || false).unwrap().to_vec();
    assert_eq!(facts.len(), 2);
    assert_eq!(known_type(&mut analysis, &facts[0]), "String");
    assert!(matches!(
        facts[1].value_type,
        V::Unavailable(U::IncompleteSyntax)
    ));
}

#[test]
fn partial_when_calls_share_overloads_in_bodies_guards_and_condition_inputs() {
    let declarations = "fun predicate(value: Boolean, next: Boolean): Boolean = next\nfun predicate(value: String, next: Boolean): Boolean = next\n";
    for editing in [
        "fun editing(value: Any) { when (value) { is String -> predicate(value,",
        "fun editing(source: (Boolean, String)) { when (source) { case (flag, value) if predicate(value,",
        "fun editing(value: Any) { when { value !is String, predicate(value,",
    ] {
        let candidates = call_candidates(&format!("{declarations}{editing}"));
        assert_eq!(candidates.len(), 1, "{editing}");
        assert_eq!(
            candidates[0].parameters[0].type_label, "String",
            "{editing}"
        );
    }
}

#[test]
fn cancelled_when_queries_do_not_commit_subject_or_pattern_scopes() {
    let mut analysis = analyze(
        "fun editing(source: (Boolean, String)) { when (source) { case (flag, value) -> value. } }",
    );
    let id = function(&analysis, "editing");
    let checks = std::cell::Cell::new(0);
    assert!(matches!(
        analysis.receivers(id, || {
            checks.set(checks.get() + 1);
            checks.get() == 7
        }),
        Err(EditingCancelled)
    ));
    assert!(!analysis.recovered_facts.contains_key(&id));
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "String");
}
