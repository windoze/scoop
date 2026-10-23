use super::*;

#[test]
fn failed_initializers_preserve_completed_calls_but_not_partial_local_declarations() {
    let text = "fun pass(value: Boolean): Boolean = value\nval broken: Boolean = if (true) { class Local(val hidden: Boolean) { val copied: Boolean get() = hidden }; val done = pass(true); missing } else false\nval healthy: Boolean = pass(false)";
    let mut analysis = analyze_initializers(text);
    let broken = region(&mut analysis, text, "pass(true)");
    assert!(matches!(
        analysis.source_body_facts(broken.id, || false).unwrap(),
        V::Unavailable(U::BodyFailure)
    ));
    let V::Known(calls) = analysis.calls(broken.id, || false).unwrap() else {
        panic!("completed call before the error")
    };
    assert_eq!(calls.len(), 1);
    let healthy = region(&mut analysis, text, "pass(false)");
    assert_eq!(facts(&mut analysis, &healthy).callables.len(), 1);
    let index = analysis.reference_index(|| false).unwrap();
    assert_eq!(index.unavailable.len(), 1, "{:?}", index.unavailable);
    for (needle, name) in [
        ("hidden:", "hidden"),
        ("copied:", "copied"),
        ("done =", "done"),
    ] {
        let location = at("src/initializers.scoop", text, needle, name.len());
        assert!(index.targets_at(&location).is_empty(), "{name}");
    }
}

#[test]
fn cancelling_an_initializer_discards_its_inference_and_allows_a_clean_retry() {
    let text = "fun pass(value: Boolean): Boolean = value\nval result = if (true) { val saved = pass(true); saved } else false";
    let mut analysis = analyze_initializers(text);
    let body = analysis
        .source_bodies()
        .into_iter()
        .find(|body| matches!(body.owner, Owner::Initialization(_)))
        .unwrap();
    let checks = std::rc::Rc::new(std::cell::Cell::new(0));
    let observed = checks.clone();
    assert!(matches!(
        analysis.source_body_facts(body.id, move || {
            observed.set(observed.get() + 1);
            observed.get() >= 4
        }),
        Err(EditingCancelled)
    ));
    assert!(checks.get() >= 4);
    assert!(!analysis.body_facts.contains_key(&body.id));
    let facts = facts(&mut analysis, &body);
    assert_eq!(facts.callables.len(), 1);
    assert!(analysis.diagnostics().is_empty());
}

#[test]
fn initializer_result_and_effect_errors_do_not_invent_an_available_body() {
    for text in [
        "fun pass(): String = \"wrong\"\nval result: Boolean = pass()",
        "suspend fun delayed(): Boolean = true\nval result: Boolean = delayed()",
        "class Box<T>(val value: T)\nval <T> Box<T>.bad: T by this",
    ] {
        let mut analysis = analyze_initializers(text);
        let body = analysis
            .source_bodies()
            .into_iter()
            .find(|body| matches!(body.owner, Owner::Initialization(_)))
            .unwrap();
        assert!(
            matches!(
                analysis.source_body_facts(body.id, || false).unwrap(),
                V::Unavailable(U::BodyFailure)
            ),
            "{text}"
        );
        assert!(!analysis.diagnostics().is_empty(), "{text}");
    }
}
