use super::*;

#[test]
fn failed_field_inference_does_not_block_independent_initializers() {
    let text = "fun pick(value: Boolean): Boolean = value\nclass State { val broken = missing; val healthy: Boolean = pick(true); fun method(): Boolean = true }";
    let mut analysis = analyze_construction(text);
    let healthy = region(&mut analysis, text, "pick(true)");
    assert_eq!(facts(&mut analysis, &healthy).callables.len(), 1);
    let failed = region(&mut analysis, text, "missing");
    assert!(matches!(
        analysis.source_body_facts(failed.id, || false).unwrap(),
        V::Unavailable(U::BodyFailure)
    ));
    let method = function(&analysis, "method");
    assert!(matches!(
        analysis.function_facts(method, || false).unwrap(),
        V::Known(_)
    ));
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.contains(&failed.origin));
}

#[test]
fn init_blocks_use_the_real_field_initialization_order() {
    let text = "class Early { init { val bad = later }; val later: Boolean = true; init { val good = later } }";
    let mut analysis = analyze_construction(text);
    let bad = region(&mut analysis, text, "bad = later");
    assert!(matches!(
        analysis.source_body_facts(bad.id, || false).unwrap(),
        V::Unavailable(U::BodyFailure)
    ));
    assert!(
        analysis
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not initialized"))
    );
    let good = region(&mut analysis, text, "good = later");
    let observed = facts(&mut analysis, &good);
    assert_eq!(
        observed.properties[0].definition,
        Some(location(text, "later: Boolean", 5))
    );
}

#[test]
fn complete_initialization_remains_available_beside_a_recovered_method() {
    let text = "class Payload(val flag: Boolean)\nclass State(seed: Payload) { val known: Boolean = seed.flag; fun damaged(value: Payload) { value. } }";
    let mut analysis = analyze_construction(text);
    let body = region(&mut analysis, text, "seed.flag");
    assert_eq!(facts(&mut analysis, &body).properties.len(), 1);
    let index = analysis.reference_index(|| false).unwrap();
    assert!(!index.unavailable.is_empty());
    assert!(!index.targets_at(&location(text, "seed.flag", 4)).is_empty());
}

#[test]
fn failed_initializers_leave_other_regions_and_methods_queryable() {
    let text = "fun pick(value: Boolean): Boolean = value\nclass State { val broken: Boolean = missing; val healthy: Boolean = pick(true); fun healthyMethod(): Boolean = true }";
    let mut analysis = analyze_construction(text);
    let failed = region(&mut analysis, text, "missing");
    assert!(matches!(
        analysis.source_body_facts(failed.id, || false).unwrap(),
        V::Unavailable(U::BodyFailure)
    ));
    let healthy = region(&mut analysis, text, "pick(true)");
    assert_eq!(facts(&mut analysis, &healthy).callables.len(), 1);
    let method = function(&analysis, "healthyMethod");
    assert!(matches!(
        analysis.function_facts(method, || false).unwrap(),
        V::Known(_)
    ));
    let count = analysis.diagnostics().len();
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.contains(&failed.origin));
    assert!(index.targets_at(&location(text, "missing", 7)).is_empty());
    assert_eq!(analysis.diagnostics().len(), count);
}

#[test]
fn implicit_base_failures_and_cycles_are_checked_without_fake_source_bodies() {
    for (text, message) in [
        (
            "open class Base(value: Boolean)\nclass Derived: Base",
            "no applicable candidate for constructor",
        ),
        (
            "class Cycle { constructor(value: Boolean): this(\"x\") {}; constructor(value: String): this(true) {} }",
            "constructor delegation cycle",
        ),
    ] {
        let mut analysis = analyze_construction(text);
        let unavailable = analysis.check_source_constructions(None, || false).unwrap();
        assert!(
            !unavailable.is_empty(),
            "{text}: {:?}; {:?}",
            analysis.diagnostics(),
            analysis.source_bodies()
        );
        assert!(
            analysis
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message.contains(message)),
            "{:?}",
            analysis.diagnostics()
        );
        let count = analysis.diagnostics().len();
        assert_eq!(
            analysis.check_source_constructions(None, || false).unwrap(),
            unavailable
        );
        assert_eq!(analysis.diagnostics().len(), count);
    }
}

#[test]
fn cancelled_construction_queries_discard_the_entire_candidate_and_can_retry() {
    let text = "fun pick(value: Boolean): Boolean = value\nclass State(seed: Boolean) { val active: Boolean = pick(seed); init { val copy = pick(active) } }";
    let mut analysis = analyze_construction(text);
    let body = analysis
        .source_bodies()
        .into_iter()
        .find(|body| matches!(body.id, Body::Construction(Construction::Property(_))))
        .unwrap();
    let checks = std::rc::Rc::new(std::cell::Cell::new(0));
    let cancel = checks.clone();
    assert!(
        analysis
            .source_body_facts(body.id, move || {
                cancel.set(cancel.get() + 1);
                cancel.get() >= 8
            })
            .is_err()
    );
    assert!(analysis.body_facts.is_empty());
    assert!(
        analysis
            .lowerer
            .editing_prepared_bodies
            .as_ref()
            .unwrap()
            .is_empty()
    );
    assert!(analysis.diagnostics().is_empty());
    assert_eq!(facts(&mut analysis, &body).callables.len(), 1);
    assert!(
        analysis
            .reference_index(|| false)
            .unwrap()
            .unavailable
            .is_empty()
    );
}
