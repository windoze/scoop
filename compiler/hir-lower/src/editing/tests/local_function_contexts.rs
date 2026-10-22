use super::receivers::{known_type, receiver};
use super::*;

#[test]
fn failed_local_headers_and_unknown_prefixes_do_not_reuse_outer_parameters() {
    for text in [
        "fun editing(value: Boolean) { fun local(value: Missing) { value. } }",
        "fun editing(value: Boolean) { fun local(value: String, value: Boolean) { value. } }",
        "fun <T> editing(value: T) { fun <T> local(value: T) { value. } }",
        "fun editing(value: Boolean) { fun local(value: String) {}\nfun local(value: String) { value. } }",
        "fun editing(value: Boolean) { fun local(value: String): Missing { value. } }",
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
    let mut analysis = analyze(
        "fun editing(value: Boolean) { val broken = ; fun local(value: String) { value. } }",
    );
    assert!(matches!(
        receiver(&mut analysis).value_type,
        V::Unavailable(U::IncompleteSyntax)
    ));
}

#[test]
fn local_return_and_loop_targets_never_borrow_an_outer_callable_boundary() {
    for declaration in [
        "fun local(value: String) { return@editing value. }",
        "fun local(value: String) = return value.",
        "fun local(value: String) { break@outer; value. }",
    ] {
        let text = format!("fun editing() {{ outer@ while (true) {{ {declaration} }} }}");
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
        "fun editing() { outer@ while (true) { fun local(value: String) { inner@ while (true) { break@inner }; value. } } }",
    );
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "String");
}

#[test]
fn local_suspension_uses_its_own_declaration() {
    let declaration = "suspend fun load(): String = \"known\"";
    let mut analysis = analyze(&format!(
        "{declaration}\nfun editing() {{ suspend fun local() {{ val value = load(); value. }} }}"
    ));
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "String");
    let mut analysis = analyze(&format!(
        "{declaration}\nsuspend fun editing() {{ fun local() {{ val value = load(); value. }} }}"
    ));
    assert!(matches!(
        receiver(&mut analysis).value_type,
        V::Unavailable(U::BodyFailure)
    ));
}

#[test]
fn an_unfinished_self_result_is_not_used_as_a_known_unit_type() {
    let mut analysis = analyze("fun editing() { fun local(value: String) = local(value). }");
    assert!(matches!(
        receiver(&mut analysis).value_type,
        V::Unavailable(U::BodyFailure)
    ));
}

#[test]
fn local_cancellation_discards_contexts_and_retries_without_stale_bindings() {
    let text = "fun editing() { fun <Item> local(value: Item) { val action = { value. } } }";
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
    assert_eq!(known_type(&mut analysis, &facts), "Item");
    assert_eq!(
        facts.local.as_ref().unwrap().definition.span.start as usize,
        text.find("value: Item").unwrap()
    );
}
