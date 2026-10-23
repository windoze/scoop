use super::*;

#[test]
fn a_failed_getter_keeps_completed_calls_and_the_independent_setter() {
    let text = "fun pass(value: Boolean): Boolean = value\nvar state: Boolean get() { val saved = pass(value = true); return missing }\nset(next) { val saved = pass(next) }\nfun healthy(): Boolean = true";
    let mut analysis = analyze_accessors(text);
    let getter = region(&mut analysis, text, "pass(value = true)");
    assert!(matches!(
        analysis.function_facts(getter.function, || false).unwrap(),
        V::Unavailable(U::BodyFailure)
    ));
    let V::Known(calls) = analysis.calls(getter.function, || false).unwrap() else {
        panic!("completed calls precede the failed expression")
    };
    assert_eq!(calls.len(), 1);
    let setter = region(&mut analysis, text, "pass(next)");
    assert_eq!(facts(&mut analysis, &setter).callables.len(), 1);
    let healthy = function(&analysis, "healthy");
    assert!(matches!(
        analysis.function_facts(healthy, || false).unwrap(),
        V::Known(_)
    ));
    let hidden = at("src/accessors.scoop", text, "saved = pass(value", 5);
    let index = analysis.reference_index(|| false).unwrap();
    assert_eq!(index.unavailable.len(), 1);
    assert!(index.targets_at(&hidden).is_empty());
    let state = at("src/accessors.scoop", text, "state:", 5);
    assert!(matches!(index.targets_at(&state), [Target::Property(_)]));
}

#[test]
fn failed_parent_bodies_do_not_publish_partial_local_accessor_or_constructor_declarations() {
    for text in [
        "val state: Boolean get() { class Local(val hidden: Boolean) { val projected: Boolean get() = hidden }; return missing }",
        "fun broken(): Boolean { class Local(val hidden: Boolean) { val projected: Boolean get() = hidden }; return missing }",
    ] {
        let mut analysis = analyze_accessors(text);
        let index = analysis.reference_index(|| false).unwrap();
        assert_eq!(
            index.unavailable.len(),
            1,
            "{text}: {:?}",
            index.unavailable
        );
        for needle in ["hidden:", "projected:"] {
            let name = needle.trim_end_matches(':');
            let location = at("src/accessors.scoop", text, needle, name.len());
            assert!(index.targets_at(&location).is_empty(), "{text}: {name}");
        }
        assert_eq!(analysis.source_bodies().len(), 1, "{text}");
    }
}

#[test]
fn accessor_cancellation_discards_body_facts_and_can_retry_without_stale_errors() {
    let text = "var state: Boolean = true\nget() { val saved = field; return saved }\nset(next) { field = next }";
    let mut analysis = analyze_accessors(text);
    let getter = analysis
        .source_bodies()
        .into_iter()
        .find(|body| {
            matches!(
                body.callable(),
                Some(hir::SemanticCallable::CurrentGetter(_))
            )
        })
        .unwrap();
    let checks = std::rc::Rc::new(std::cell::Cell::new(0));
    let observed = checks.clone();
    let result = analysis.function_facts(getter.function, move || {
        observed.set(observed.get() + 1);
        observed.get() >= 4
    });
    assert!(matches!(result, Err(EditingCancelled)));
    assert!(checks.get() >= 4);
    assert!(!analysis.body_facts.contains_key(&getter.function));
    assert!(!facts(&mut analysis, &getter).locals.is_empty());
    assert!(analysis.diagnostics().is_empty());
    assert!(analysis.reference_index(|| true).is_err());
    assert!(analysis.reference_index.is_none());
    assert!(
        analysis
            .reference_index(|| false)
            .unwrap()
            .unavailable
            .is_empty()
    );
}
