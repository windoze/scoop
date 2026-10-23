use super::*;

#[test]
fn delegated_property_initializers_only_record_actual_source_calls() {
    let text = "class Slot(val value: Boolean) { operator fun provideDelegate(): Slot = this; operator fun getValue(thisRef: State): Boolean = value }\nfun pick(value: Boolean): Boolean = value\nclass State(seed: Boolean) { val active by Slot(value = pick(seed)) }";
    let mut analysis = analyze_construction(text);
    let body = region(&mut analysis, text, "Slot(value = pick(seed))");
    let observed = facts(&mut analysis, &body);
    assert_eq!(observed.callables.len(), 2, "{:?}", observed.callables);
    let V::Known(calls) = analysis.calls(body.id, || false).unwrap() else {
        panic!("delegate expression calls")
    };
    assert_eq!(calls.len(), 2);
    assert!(
        calls
            .iter()
            .all(|call| call.name.as_deref() != Some("provideDelegate"))
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
}

#[test]
fn shared_inner_initializers_produce_one_set_of_source_bindings() {
    let text = "fun pick(value: Boolean): Boolean = value\nclass Outer(val flag: Boolean) { inner class Inner { init { val local = pick(flag); val copy = local }; constructor(value: Boolean) {}; constructor(value: String) {} } }";
    let mut analysis = analyze_construction(text);
    let body = region(&mut analysis, text, "pick(flag)");
    let observed = facts(&mut analysis, &body);
    let local = observed
        .locals
        .iter()
        .find(|local| {
            local.reference.name == "local" && local.role == hir::EditingLocalRole::Declaration
        })
        .unwrap();
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let targets = index.targets_at(&local.reference.definition);
    assert_eq!(targets.len(), 1);
    assert_eq!(index.references_to(targets[0]).count(), 2);
    let pick = index.targets_at(&location(text, "pick(value: Boolean)", 4))[0];
    assert_eq!(index.references_to(pick).count(), 2);
}

#[test]
fn repeated_common_initialization_does_not_register_duplicate_local_default_regions() {
    let text = "fun pick(value: Boolean): Boolean = value\nclass Outer(val flag: Boolean) { inner class Inner { init { val local = pick(flag); fun chosen(value: Boolean = local): Boolean = value; class Local(val value: Boolean = chosen()); val instance = Local(); val result = chosen() }; constructor(value: Boolean) {}; constructor(value: String) {} } }";
    let mut analysis = analyze_construction(text);
    let body = region(&mut analysis, text, "pick(flag)");
    let observed = facts(&mut analysis, &body);
    let local = observed
        .locals
        .iter()
        .find(|local| {
            local.reference.name == "local" && local.role == hir::EditingLocalRole::Declaration
        })
        .unwrap();
    let defaults = analysis
        .source_bodies()
        .into_iter()
        .filter(|body| matches!(body.id, Body::Default(_)))
        .collect::<Vec<_>>();
    assert_eq!(defaults.len(), 2, "{defaults:?}");
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let targets = index.targets_at(&local.reference.definition);
    assert_eq!(targets.len(), 1);
    assert_eq!(index.references_to(targets[0]).count(), 2);
    let targets = index.targets_at(&location(text, "chosen()", 6));
    assert_eq!(targets.len(), 1);
}

#[test]
fn companion_base_delegation_uses_the_hosts_generic_parameters() {
    let text = "open class Base<T>\nclass Host<T> { companion object: Base<T>() { val flag: Boolean = true } }";
    let mut analysis = analyze_construction(text);
    let body = region(&mut analysis, text, "Base<T>()");
    assert!(matches!(
        body.owner,
        hir::EditingSourceBodyOwner::Initialization(_)
    ));
    assert_eq!(facts(&mut analysis, &body).callables.len(), 1);
    assert!(
        analysis
            .reference_index(|| false)
            .unwrap()
            .unavailable
            .is_empty()
    );
}

#[test]
fn constructor_lambdas_and_selectors_keep_real_parameters_and_type_contexts() {
    let text = "class Payload(val flag: Boolean)\nclass Reader(seed: Payload) { val flag: Boolean = seed.flag; val read: () -> Boolean = { seed.flag } }";
    let mut analysis = analyze_construction(text);
    let body = region(&mut analysis, text, "seed.flag; val read");
    let observed = facts(&mut analysis, &body);
    let parameter = observed
        .locals
        .iter()
        .find(|local| local.reference.name == "seed")
        .unwrap();
    assert_eq!(local_type(&mut analysis, &body, parameter), "Payload");
    let V::Known(selectors) = analysis.selectors(body.id, || false).unwrap() else {
        panic!("checked constructor selector")
    };
    assert_eq!(
        selectors[0].local.as_ref().unwrap().definition,
        parameter.reference.definition
    );
    let body = region(&mut analysis, text, "{ seed.flag }");
    let observed = facts(&mut analysis, &body);
    assert!(
        observed
            .locals
            .iter()
            .any(|local| local.reference.definition == parameter.reference.definition)
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let target = index.targets_at(&parameter.reference.definition)[0];
    assert_eq!(index.references_to(target).count(), 3);
}

#[test]
fn object_and_companion_regions_use_the_original_initialization_units() {
    for text in [
        "fun pick(value: Boolean): Boolean = value\nobject State { val active: Boolean = pick(true); init { val copy = pick(active) } }",
        "fun pick(value: Boolean): Boolean = value\nclass Host<T> { companion object { val active: Boolean = pick(true); init { val copy = pick(active) } } }",
    ] {
        let mut analysis = analyze_construction(text);
        let property = region(&mut analysis, text, "pick(true)");
        assert!(matches!(
            property.owner,
            hir::EditingSourceBodyOwner::Initialization(_)
        ));
        assert!(property.callee().is_none());
        assert_eq!(facts(&mut analysis, &property).callables.len(), 1);
        let init = region(&mut analysis, text, "pick(active)");
        assert!(matches!(
            init.owner,
            hir::EditingSourceBodyOwner::Initialization(_)
        ));
        assert_eq!(facts(&mut analysis, &init).properties.len(), 1);
        let index = analysis.reference_index(|| false).unwrap();
        assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    }
}

#[test]
fn local_class_initialization_keeps_outer_captures_and_is_not_lowered_again() {
    let text = "fun pick(value: Boolean): Boolean = value\nfun outer(flag: Boolean): Boolean { val captured = flag; class Local { val value: Boolean = pick(captured) }; val one = Local(); return one.value }";
    let mut analysis = analyze_construction(text);
    let outer = function(&analysis, "outer");
    assert!(
        matches!(
            analysis.function_facts(outer, || false).unwrap(),
            V::Known(_)
        ),
        "{:?}",
        analysis.diagnostics()
    );
    let count = analysis.lowerer.class_constructors.len();
    let body = region(&mut analysis, text, "pick(captured)");
    let observed = facts(&mut analysis, &body);
    let capture = observed
        .locals
        .iter()
        .find(|local| local.reference.name == "captured")
        .unwrap();
    assert_eq!(
        capture.reference.definition,
        location(text, "captured = flag", 8)
    );
    assert_eq!(local_type(&mut analysis, &body, capture), "Boolean");
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let target = index.targets_at(&capture.reference.definition)[0];
    assert_eq!(index.references_to(target).count(), 2);
    assert_eq!(analysis.lowerer.class_constructors.len(), count);
}

#[test]
fn interface_delegate_expressions_have_their_own_source_regions() {
    let text = "interface Readable { fun read(): Boolean }\nclass Value: Readable { override fun read(): Boolean = true }\nfun pick(value: Readable): Readable = value\nclass Wrapper(source: Readable): Readable by pick(source)";
    let mut analysis = analyze_construction(text);
    let body = region(&mut analysis, text, "pick(source)");
    assert!(matches!(
        body.id,
        Body::Construction(Construction::InterfaceDelegate(_))
    ));
    let observed = facts(&mut analysis, &body);
    assert_eq!(observed.callables.len(), 1);
    let source = observed
        .locals
        .iter()
        .find(|local| local.reference.name == "source")
        .unwrap();
    assert_eq!(
        source.reference.definition,
        location(text, "source: Readable", 6)
    );
    assert!(
        analysis
            .reference_index(|| false)
            .unwrap()
            .unavailable
            .is_empty()
    );
}

#[test]
fn initialization_writes_keep_property_identity_separate_from_constructor_parameters() {
    let text = "class State(var active: Boolean) { init { this.active = false; val before = active; val after = this.active } }";
    let mut analysis = analyze_construction(text);
    let body = region(&mut analysis, text, "this.active =");
    let observed = facts(&mut analysis, &body);
    assert_eq!(observed.properties.len(), 2);
    assert_eq!(
        observed.properties[0].access,
        hir::EditingPropertyAccess::Write
    );
    assert_eq!(
        observed.properties[1].access,
        hir::EditingPropertyAccess::Read
    );
    assert_eq!(observed.properties[0].target, observed.properties[1].target);
    let parameter = observed
        .locals
        .iter()
        .find(|local| local.span == location(text, "active; val after", 6).span)
        .unwrap();
    assert_eq!(
        analysis.body_parameter(&body, &parameter.reference.definition),
        Some((body.callee().unwrap(), 0))
    );
}
