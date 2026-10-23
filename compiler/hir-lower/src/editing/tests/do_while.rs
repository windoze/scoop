use super::candidates::call_candidates;
use super::receivers::{known_type, receiver};
use super::*;

#[test]
fn do_bodies_have_loop_scope_without_postcondition_refinements() {
    for (text, expected) in [
        (
            "fun editing(value: Any) { do { value. } while (value is String) }",
            "Any",
        ),
        (
            "fun editing(value: Boolean) { cycle@ do { if (false) continue@cycle; value. } while (true) }",
            "Boolean",
        ),
        (
            "fun editing(value: String) { do value. while (true) }",
            "String",
        ),
        (
            "fun editing(value: Any) { do { val local = true; local. } while (false) }",
            "Boolean",
        ),
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), expected, "{text}");
    }
}

#[test]
fn do_conditions_see_only_direct_initialized_body_bindings() {
    for text in [
        "fun editing() { do { val value = \"ready\" } while (value.) }",
        "fun editing() { do val value = \"ready\" while (value.) }",
        "fun editing(value: Boolean) { do { val value = \"ready\" } while (value.) }",
        "fun editing() { do { val (flag, value) = (true, \"ready\") } while (value.) }",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
        let definition = facts.local.unwrap().definition.span;
        assert_eq!(
            &text[definition.start as usize..definition.end as usize],
            "value"
        );
    }
    let mut analysis =
        analyze("fun editing() { do { { val value = \"hidden\" } } while (value.) }");
    assert!(matches!(
        receiver(&mut analysis).value_type,
        V::Unavailable(U::BodyFailure)
    ));
}

#[test]
fn do_condition_initialization_uses_continue_targets_and_finally_outcomes() {
    for (prefix, available) in [
        ("if (flag) continue@cycle; val value = \"ready\"", false),
        ("val value = \"ready\"; if (flag) continue@cycle", true),
        ("while (flag) { continue }; val value = \"ready\"", true),
        (
            "if (flag) { try { continue@cycle } finally { break@cycle } }; val value = \"ready\"",
            true,
        ),
        (
            "try { if (flag) continue@cycle } finally {}; val value = \"ready\"",
            false,
        ),
    ] {
        let text =
            format!("fun editing(flag: Boolean) {{ cycle@ do {{ {prefix} }} while (value.) }}");
        let mut analysis = analyze(&text);
        let facts = receiver(&mut analysis);
        if available {
            assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
        } else {
            assert!(
                matches!(facts.value_type, V::Unavailable(U::BodyFailure)),
                "{text}: {facts:?}"
            );
        }
    }
}

#[test]
fn an_unknown_or_failed_do_body_does_not_supply_a_condition_scope() {
    let mut analysis = analyze("fun editing() { do { val value = true; value. } while (value.) }");
    let function = function(&analysis, "editing");
    let facts = analysis.receivers(function, || false).unwrap().to_vec();
    assert_eq!(facts.len(), 2);
    assert_eq!(known_type(&mut analysis, &facts[0]), "Boolean");
    assert!(matches!(
        facts[1].value_type,
        V::Unavailable(U::IncompleteSyntax)
    ));
    for text in [
        "fun editing() { do { val value = missing } while (value.) }",
        "fun editing(value: Boolean) { do { continue@missing; value. } while (true) }",
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
fn partial_do_condition_calls_use_body_bindings_and_original_overloads() {
    let candidates = call_candidates(
        "fun predicate(value: Boolean, next: Boolean): Boolean = next\nfun predicate(value: String, next: Boolean): Boolean = next\nfun editing() { do { val value = \"ready\" } while (predicate(value,",
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].parameters[0].type_label, "String");
}

#[test]
fn do_recovery_does_not_carry_mutable_facts_across_an_unknown_backedge() {
    for (text, expected) in [
        (
            "fun editing(value: Any) { var local: Any = value; if (local is String) { do { local. } while (true) } }",
            "Any",
        ),
        (
            "fun editing(value: Any) { if (value is String) { do { value. } while (true) } }",
            "String",
        ),
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), expected, "{text}");
    }
}

#[test]
fn cancelling_do_condition_queries_restores_scope_and_initialization_state() {
    let mut analysis = analyze("fun editing() { do { val value = \"ready\" } while (value.) }");
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
