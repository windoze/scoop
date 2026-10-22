use super::*;

fn receiver(analysis: &mut DeclarationAnalysis) -> hir::EditingReceiver {
    let id = function(analysis, "editing");
    let facts = analysis.receivers(id, || false).unwrap();
    assert_eq!(facts.len(), 1, "{facts:?}");
    facts[0].clone()
}

fn known_type(analysis: &mut DeclarationAnalysis, receiver: &hir::EditingReceiver) -> String {
    let V::Known(ty) = receiver.value_type else {
        panic!(
            "receiver must have a complete type: {receiver:?}; {:?}",
            analysis.diagnostics()
        );
    };
    let id = function(analysis, "editing");
    let V::Known(description) = analysis.describe_type(id, ty).unwrap() else {
        panic!("receiver type is describable in the function context");
    };
    description.name
}

#[test]
fn parameter_receiver_uses_its_real_type_and_definition_in_both_body_forms() {
    for text in [
        "fun editing(value: Boolean) { value. }",
        "fun editing(value: Boolean) = value.",
        "fun editing(value: Boolean) { value.\n\n",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "Boolean");
        let local = facts.local.unwrap();
        assert_eq!(
            local.definition.span.start as usize,
            text.find("value").unwrap()
        );
        assert_eq!(local.name, "value");
        assert_eq!(
            local.definition.source.logical_path().as_str(),
            "src/editing.scoop"
        );
    }
}

#[test]
fn prefix_locals_shadow_parameters_through_the_ordinary_scope() {
    let text = "fun editing(value: Boolean) {\nval value = \"local\"\nvalue.\n}";
    let mut analysis = analyze(text);
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "String");
    let definition = facts.local.unwrap().definition;
    assert_eq!(
        definition.span.start as usize,
        text.find("value =").unwrap()
    );
}

#[test]
fn generic_receiver_keeps_the_original_binder() {
    let mut analysis = analyze("fun <T> editing(value: T) { val copy = value\ncopy. }");
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "T");
}

#[test]
fn complete_prefix_calls_use_real_signatures_without_finishing_all_bodies() {
    let mut analysis = analyze(
        "fun identity(value: Boolean): Boolean = value\nfun editing() { val copy = identity(true)\ncopy. }",
    );
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "Boolean");
}

#[test]
fn a_pending_called_result_does_not_become_a_unit_receiver() {
    let mut analysis = analyze("fun editing() { pending(true). }\nfun pending(value: Boolean) =");
    let facts = receiver(&mut analysis);
    assert!(
        matches!(facts.value_type, V::Unavailable(U::BodyFailure)),
        "{facts:?}"
    );
}

#[test]
fn complete_inference_dependencies_use_their_actual_bodies() {
    for text in [
        "fun result() = true\nfun editing() { result(). }",
        "val result = true\nfun editing() { result. }",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "Boolean");
    }
}

#[test]
fn failed_property_inference_does_not_expose_its_pending_type() {
    let mut analysis = analyze("val result = missing\nfun editing() { result. }");
    let facts = receiver(&mut analysis);
    assert!(
        matches!(facts.value_type, V::Unavailable(U::BodyFailure)),
        "{facts:?}"
    );
}

#[test]
fn unknown_prefix_and_inner_scopes_do_not_reuse_outer_bindings() {
    for text in [
        "fun editing(value: Boolean) { val missing =\nvalue. }",
        "fun editing(value: Boolean) { if (value) { value. } }",
        "fun editing(value: Boolean) { invoke { value -> value. } }",
        "fun editing(value: Boolean) = invoke { value -> value. }",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert!(
            matches!(facts.value_type, V::Unavailable(U::IncompleteSyntax)),
            "{text}: {facts:?}"
        );
        assert!(facts.local.is_none());
    }
}

#[test]
fn semantic_failure_in_the_prefix_or_receiver_stays_unavailable() {
    for text in [
        "fun editing(value: Boolean) { val copy = missing\nvalue. }",
        "fun editing(value: Boolean) { missing. }",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert!(
            matches!(facts.value_type, V::Unavailable(U::BodyFailure)),
            "{facts:?}"
        );
        assert!(facts.local.is_none());
    }
}

#[test]
fn cancellation_discards_partial_scopes_and_allows_a_clean_retry() {
    let mut analysis = analyze("fun editing(value: Boolean) { val copy = value\ncopy. }");
    let id = function(&analysis, "editing");
    let checks = std::cell::Cell::new(0);
    let cancelled = analysis.receivers(id, || {
        checks.set(checks.get() + 1);
        checks.get() == 3
    });
    assert!(matches!(cancelled, Err(EditingCancelled)));
    assert!(!analysis.receiver_facts.contains_key(&id));
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "Boolean");
    assert!(analysis.diagnostics().is_empty());
    let cached = analysis.receivers(id, || false).unwrap()[0]
        .local
        .as_ref()
        .unwrap()
        .binding;
    assert_eq!(facts.local.unwrap().binding, cached);
}
