use super::*;

#[test]
fn a_failed_default_keeps_independent_signatures_bodies_and_completed_calls() {
    let text = "fun helper(value: Boolean): Boolean = value\nfun never(broken: Boolean = if (true) helper(true) else missing, healthy: Boolean = helper(false)): Boolean = healthy";
    let mut analysis = analyze_defaults(text);
    let failed = region(&mut analysis, text, "if (true) helper(true) else missing");
    assert!(matches!(
        analysis.source_body_facts(failed.id, || false).unwrap(),
        V::Unavailable(U::BodyFailure)
    ));
    let V::Known(calls) = analysis.calls(failed.id, || false).unwrap() else {
        panic!("independently completed call")
    };
    assert_eq!(calls.len(), 1);
    let healthy = region(&mut analysis, text, "helper(false)");
    assert_eq!(facts(&mut analysis, &healthy).callables.len(), 1);
    let never = function(&analysis, "never");
    assert!(matches!(analysis.signature(never), V::Known(_)));
    assert!(matches!(
        analysis.function_facts(never, || false).unwrap(),
        V::Known(_)
    ));
    let index = analysis.reference_index(|| false).unwrap();
    assert_eq!(index.unavailable, vec![failed.origin]);
    assert!(index.targets_at(&location(text, "missing", 7)).is_empty());
}

#[test]
fn cancelled_default_queries_discard_templates_and_observations_and_can_retry() {
    let text = "fun helper(value: Boolean): Boolean = value\nfun never(seed: Boolean, value: Boolean = helper(seed)): Boolean = value";
    let mut analysis = analyze_defaults(text);
    let source = analysis
        .source_bodies()
        .into_iter()
        .find(|body| matches!(body.id, Body::Default(_)))
        .unwrap();
    let Body::Default(key) = source.id else {
        unreachable!()
    };
    let count = analysis.lowerer.export_default_exprs.len();
    let checks = std::rc::Rc::new(std::cell::Cell::new(0));
    let cancel = checks.clone();
    assert!(
        analysis
            .source_body_facts(source.id, move || {
                cancel.set(cancel.get() + 1);
                cancel.get() >= 5
            })
            .is_err()
    );
    assert!(checks.get() >= 5);
    assert!(!analysis.body_facts.contains_key(&source.id));
    assert!(
        !analysis
            .lowerer
            .default_templates
            .contains_key(&key.tuple())
    );
    assert_eq!(analysis.lowerer.export_default_exprs.len(), count);
    assert!(analysis.diagnostics().is_empty());
    assert_eq!(facts(&mut analysis, &source).callables.len(), 1);
}

#[test]
fn cycles_use_the_existing_default_diagnostic_and_leave_other_defaults_queryable() {
    let text = "fun first(value: Boolean = second()): Boolean = value\nfun second(value: Boolean = first()): Boolean = value\nfun healthy(value: Boolean = true): Boolean = value";
    let mut analysis = analyze_defaults(text);
    let cycle = region(&mut analysis, text, "second()");
    assert!(matches!(
        analysis.source_body_facts(cycle.id, || false).unwrap(),
        V::Unavailable(_)
    ));
    assert!(
        analysis
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cyclic default expansion")),
        "{:?}",
        analysis.diagnostics()
    );
    let healthy = region(&mut analysis, text, "true");
    assert!(!facts(&mut analysis, &healthy).expressions.is_empty());
}

#[test]
fn a_complete_default_remains_queryable_when_its_function_body_is_incomplete() {
    let text = "fun helper(value: Boolean): Boolean = value\nfun never(seed: Boolean, value: Boolean = helper(seed)): Boolean { seed. }";
    let mut analysis = analyze_defaults(text);
    let body = region(&mut analysis, text, "helper(seed)");
    assert_eq!(facts(&mut analysis, &body).callables.len(), 1);
    assert!(analysis.diagnostics().is_empty());
    let index = analysis.reference_index(|| false).unwrap();
    assert!(!index.unavailable.is_empty());
    assert!(
        !index
            .targets_at(&location(text, "helper(seed)", 6))
            .is_empty()
    );
}
