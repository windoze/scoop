use super::candidates::call_candidates;
use super::receivers::{known_type, receiver};
use super::*;

#[test]
fn complete_conditions_narrow_actual_block_and_statement_branches() {
    for text in [
        "fun editing(value: Any) { if (value is String) { value. } }",
        "fun editing(value: Any) { if (value !is String) {} else { value. } }",
        "fun editing(value: Any) { if (value is String) value. }",
        "fun editing(value: Any) { if (true) {} else if (value is String) { value. } }",
        "fun editing(value: Any) { val result = if (value is String) { value. } else false }",
        "fun editing(value: Any) = if (value is String) { value. } else false",
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
fn branches_keep_independent_bindings_failures_and_flow_states() {
    for (text, first) in [
        (
            "fun editing(value: Boolean) { if (value) { val value = \"inner\"; value. } else { value. }; value. }",
            Some("String"),
        ),
        (
            "fun editing(value: Boolean) { if (value) { val copy = missing; value. } else { value. }; value. }",
            None,
        ),
    ] {
        let mut analysis = analyze(text);
        let id = function(&analysis, "editing");
        let facts = analysis.receivers(id, || false).unwrap().to_vec();
        assert_eq!(facts.len(), 3);
        if let Some(name) = first {
            assert_eq!(known_type(&mut analysis, &facts[0]), name);
        } else {
            assert!(matches!(
                facts[0].value_type,
                V::Unavailable(U::BodyFailure)
            ));
        }
        assert_eq!(known_type(&mut analysis, &facts[1]), "Boolean");
        assert!(matches!(
            facts[2].value_type,
            V::Unavailable(U::IncompleteSyntax)
        ));
    }
    let mut analysis = analyze(
        "fun editing(value: Any) { var local: Any = value; if (local is String) { local = true; local. } else { local. } }",
    );
    let id = function(&analysis, "editing");
    let facts = analysis.receivers(id, || false).unwrap().to_vec();
    assert_eq!(facts.len(), 2);
    assert_ne!(known_type(&mut analysis, &facts[0]), "String");
    assert_eq!(known_type(&mut analysis, &facts[1]), "Any");
}

#[test]
fn conditions_use_known_input_but_do_not_publish_unchecked_branch_facts() {
    let mut analysis = analyze("fun editing(value: Any) { if (value.) { value. } }");
    let id = function(&analysis, "editing");
    let facts = analysis.receivers(id, || false).unwrap().to_vec();
    assert_eq!(facts.len(), 1);
    assert_eq!(known_type(&mut analysis, &facts[0]), "Any");
    let mut analysis = analyze("fun editing(value: Any) { if (check(,)) { value. } }");
    assert!(matches!(
        receiver(&mut analysis).value_type,
        V::Unavailable(U::IncompleteSyntax)
    ));
    for condition in ["missing", "\"not Boolean\""] {
        let text = format!("fun editing(value: Boolean) {{ if ({condition}) {{ value. }} }}");
        let mut analysis = analyze(&text);
        assert!(matches!(
            receiver(&mut analysis).value_type,
            V::Unavailable(U::BodyFailure)
        ));
    }
}

#[test]
fn incomplete_branch_calls_use_the_actual_narrowed_argument() {
    let candidates = call_candidates(
        "fun choose(value: Boolean, text: String): String = text\nfun choose(value: String, text: String): String = text\nfun editing(value: Any) { if (value is String) { choose(value,",
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].parameters[0].type_label, "String");
}

#[test]
fn cancelling_a_branch_query_does_not_commit_its_scope_or_condition() {
    let mut analysis = analyze("fun editing(value: Any) { if (value is String) { value. } }");
    let id = function(&analysis, "editing");
    let checks = std::cell::Cell::new(0);
    assert!(matches!(
        analysis.receivers(id, || {
            checks.set(checks.get() + 1);
            checks.get() == 6
        }),
        Err(EditingCancelled)
    ));
    assert!(!analysis.recovered_facts.contains_key(&id));
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "String");
}
