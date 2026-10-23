use super::candidates::call_candidates;
use super::receivers::{known_type, receiver};
use super::*;

#[test]
fn unfinished_try_bodies_keep_their_prefix_even_before_a_handler_is_written() {
    for text in [
        "fun editing(value: String) { try { value.",
        "fun editing(value: String) { try { value. } }",
        "fun editing(value: Any) { try { val local = \"ready\"; local. } finally {} }",
        "fun editing(value: String) { try { value. } catch (error: ) {} }",
        "fun editing(value: String) = try { value. } catch (error: Throwable) { false }",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
    }
}

#[test]
fn catch_parameters_have_independent_scopes_and_original_definition_positions() {
    let text = "fun editing(error: Boolean) { try {} catch (error: Exception) { error. } catch (error: Throwable) { error. } finally { error. } }";
    let mut analysis = analyze(text);
    let id = function(&analysis, "editing");
    let facts = analysis.receivers(id, || false).unwrap().to_vec();
    assert_eq!(facts.len(), 3);
    for (index, expected) in ["Exception", "Throwable", "Boolean"]
        .into_iter()
        .enumerate()
    {
        assert_eq!(known_type(&mut analysis, &facts[index]), expected);
    }
    for (index, marker) in [
        (0, "error: Exception"),
        (1, "error: Throwable"),
        (2, "error: Boolean"),
    ] {
        assert_eq!(
            facts[index].local.as_ref().unwrap().definition.span.start as usize,
            text.find(marker).unwrap()
        );
    }
}

#[test]
fn generic_handlers_keep_exact_type_applications_and_member_arguments() {
    let candidates = call_candidates(
        "class Failure<T>(val payload: T) : Exception()\nfun choose(value: String, next: Boolean): Boolean = next\nfun choose(value: Boolean, next: Boolean): Boolean = next\nfun editing() { try {} catch (error: Failure<String>) { choose(error.payload,",
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].parameters[0].type_label, "String");
}

#[test]
fn invalid_or_covered_catch_types_do_not_supply_parameters() {
    for text in [
        "fun editing() { try {} catch (error: String) { error. } }",
        "fun editing() { try {} catch (error: Missing) { error. } }",
        "fun editing() { try {} catch (earlier: Throwable) {} catch (error: Exception) { error. } }",
        "fun editing() { try {} catch (earlier: Exception) {} catch (error: Exception) { error. } }",
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
fn sibling_handlers_survive_unknown_bodies_without_leaking_local_names() {
    let text = "fun editing(value: String) { try { val local = missing; value. } catch (first: Exception) { val local = missing; first. } catch (second: Throwable) { second. } finally { value. } }";
    let mut analysis = analyze(text);
    let id = function(&analysis, "editing");
    let facts = analysis.receivers(id, || false).unwrap().to_vec();
    assert_eq!(facts.len(), 4);
    assert!(matches!(
        facts[0].value_type,
        V::Unavailable(U::BodyFailure)
    ));
    assert!(matches!(
        facts[1].value_type,
        V::Unavailable(U::BodyFailure)
    ));
    assert_eq!(known_type(&mut analysis, &facts[2]), "Throwable");
    assert_eq!(known_type(&mut analysis, &facts[3]), "String");
    for text in [
        "fun editing() { try { val hidden = true } finally { hidden. } }",
        "fun editing() { try {} catch (hidden: Throwable) {} finally { hidden. } }",
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
fn cancellation_restores_handler_scope_and_does_not_publish_partial_facts() {
    let mut analysis = analyze("fun editing() { try {} catch (error: Throwable) { error. } }");
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
    assert_eq!(known_type(&mut analysis, &facts), "Throwable");
}
