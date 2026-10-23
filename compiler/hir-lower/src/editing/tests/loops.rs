use super::candidates::call_candidates;
use super::receivers::{known_type, receiver};
use super::*;

#[test]
fn while_conditions_narrow_block_single_statement_and_labeled_bodies() {
    for text in [
        "fun editing(value: Any) { while (value is String) { value. } }",
        "fun editing(value: Any) { while (value is String) value. }",
        "fun editing(value: Any) { cycle@ while (value is String) { if (false) continue@cycle; value. } }",
        "fun editing(value: Any) { outer@ while (true) inner@ while (value is String) { if (false) break@outer; value. } }",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
        assert_eq!(
            facts.local.unwrap().definition.span.start as usize,
            text.find("value").unwrap()
        );
    }
}

#[test]
fn unknown_backedges_drop_mutable_facts_but_keep_values_and_condition_refinements() {
    for (text, expected) in [
        (
            "fun editing(value: Any) { var local: Any = value; if (local is String) { while (true) { local.; local = true } } }",
            "Any",
        ),
        (
            "fun editing(value: Any) { if (value is String) { while (true) { value. } } }",
            "String",
        ),
        (
            "fun editing(value: Any) { var local: Any = value; while (local is String) { local.; local = true } }",
            "String",
        ),
        (
            "fun editing(value: Any) { while (value is String) { val value = true; value. } }",
            "Boolean",
        ),
        (
            "fun editing(value: Any) { var local: Any = value; val check = local is String; if (check) { while (check) { local. } } }",
            "Any",
        ),
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), expected, "{text}");
    }
}

#[test]
fn while_failures_do_not_publish_body_or_later_statement_facts() {
    for text in [
        "fun editing(value: Boolean) { while (missing) { value. } }",
        "fun editing(value: Boolean) { while (\"wrong\") { value. } }",
        "fun editing(value: Boolean) { cycle@ while (true) { continue@missing; value. } }",
        "fun editing(value: Boolean) { while (true) { val bad = missing; value. } }",
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
    let mut analysis = analyze("fun editing(value: Boolean) { while (true) { value. }; value. }");
    let function = function(&analysis, "editing");
    let facts = analysis.receivers(function, || false).unwrap().to_vec();
    assert_eq!(facts.len(), 2);
    assert_eq!(known_type(&mut analysis, &facts[0]), "Boolean");
    assert!(matches!(
        facts[1].value_type,
        V::Unavailable(U::IncompleteSyntax)
    ));
}

#[test]
fn unfinished_conditions_and_calls_only_consume_available_input() {
    let mut analysis = analyze("fun editing(value: Any) { cycle@ while (value.) { value. } }");
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "Any");
    let candidates = call_candidates(
        "fun choose(value: Boolean, next: Boolean): Boolean = value\nfun choose(value: String, next: Boolean): String = value\nfun editing(value: Any) { while (value is String) { choose(value,",
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].parameters[0].type_label, "String");
    let candidates = call_candidates(
        "fun predicate(value: Boolean): Boolean = value\nfun editing() { cycle@ while (predicate(",
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].parameters[0].type_label, "Boolean");
}

#[test]
fn cancelling_while_recovery_does_not_publish_a_partial_loop_scope() {
    let mut analysis = analyze("fun editing(value: Any) { while (value is String) { value. } }");
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
