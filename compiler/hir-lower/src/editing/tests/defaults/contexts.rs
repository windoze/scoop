use super::*;

#[test]
fn local_function_defaults_keep_captures_from_the_enclosing_body() {
    let text = "fun outer(seed: Boolean): Boolean { val saved = seed; fun local(value: Boolean = saved): Boolean = value; return local() }";
    let mut analysis = analyze_defaults(text);
    let body = region(&mut analysis, text, "saved)");
    let observed = facts(&mut analysis, &body);
    let saved = observed
        .locals
        .iter()
        .find(|local| local.reference.name == "saved" && local.role == hir::EditingLocalRole::Read)
        .unwrap();
    assert_eq!(local_type(&mut analysis, &body, saved), "Boolean");
    assert_eq!(saved.reference.definition, location(text, "saved =", 5));
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let target = index.targets_at(&saved.reference.definition)[0];
    assert!(matches!(target, Target::Binding(_)));
    assert_eq!(index.references_to(target).count(), 2);
}

#[test]
fn captured_parameters_keep_the_original_owner_in_selectors_and_references() {
    let text = "class Holder(val flag: Boolean)\nfun outer(seed: Holder): Boolean { fun local(value: Boolean = seed.flag): Boolean = value; return local() }";
    let mut analysis = analyze_defaults(text);
    let body = region(&mut analysis, text, "seed.flag");
    let observed = facts(&mut analysis, &body);
    let seed = observed
        .locals
        .iter()
        .find(|local| local.reference.name == "seed" && local.role == hir::EditingLocalRole::Read)
        .unwrap();
    let outer = function(&analysis, "outer");
    assert_eq!(
        analysis.body_parameter(&body, &seed.reference.definition),
        Some((hir::SemanticCallable::Current(outer).into(), 0))
    );
    let V::Known(selectors) = analysis.selectors(body.id, || false).unwrap() else {
        panic!("captured selector")
    };
    assert_eq!(
        selectors[0].local.as_ref().unwrap().definition,
        seed.reference.definition
    );
    let index = analysis.reference_index(|| false).unwrap();
    let targets = index.targets_at(&seed.reference.definition);
    assert!(
        matches!(targets, [Target::Parameter { index: 0, .. }]),
        "{targets:?}"
    );
    assert_eq!(index.references_to(targets[0]).count(), 2);
}

#[test]
fn local_constructor_defaults_keep_the_enclosing_parameter_capture() {
    let text = "fun outer(seed: Boolean): Boolean { class Local(val value: Boolean = seed); return Local().value }";
    let mut analysis = analyze_defaults(text);
    let body = region(&mut analysis, text, "seed);");
    let observed = facts(&mut analysis, &body);
    let seed = observed
        .locals
        .iter()
        .find(|local| local.reference.name == "seed" && local.role == hir::EditingLocalRole::Read)
        .unwrap();
    assert_eq!(
        seed.reference.definition,
        location(text, "seed: Boolean", 4)
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let targets = index.targets_at(&seed.reference.definition);
    assert!(matches!(targets, [Target::Parameter { index: 0, .. }]));
    assert_eq!(index.references_to(targets[0]).count(), 2);
}

#[test]
fn default_lambdas_and_local_generics_keep_their_own_type_contexts() {
    for text in [
        "fun <T> never(seed: T, value: () -> T = { seed }): T = value()",
        "fun <T> never(seed: T, value: T = if (true) { fun <U> keep(value: U): U = value; keep(seed) } else { seed }): T = value",
    ] {
        let mut analysis = analyze_defaults(text);
        let needle = if text.contains("() ->") {
            "{ seed }"
        } else {
            "keep(seed)"
        };
        let body = region(&mut analysis, text, needle);
        let observed = facts(&mut analysis, &body);
        let seed = observed
            .locals
            .iter()
            .find(|local| {
                local.reference.name == "seed" && local.role == hir::EditingLocalRole::Read
            })
            .unwrap();
        assert_eq!(local_type(&mut analysis, &body, seed), "T");
        if let Some(local) = observed.locals.iter().find(|local| {
            local.reference.name == "value" && local.role == hir::EditingLocalRole::Read
        }) {
            assert_eq!(local_type(&mut analysis, &body, local), "U");
        }
        assert!(
            analysis
                .reference_index(|| false)
                .unwrap()
                .unavailable
                .is_empty(),
            "{text}"
        );
    }
}

#[test]
fn member_defaults_reuse_receivers_and_complete_selector_facts() {
    let text = "class Box<T>(val item: T) { fun copy(value: T = this.item): T = value }";
    let mut analysis = analyze_defaults(text);
    let body = region(&mut analysis, text, "this.item");
    let observed = facts(&mut analysis, &body);
    assert_eq!(observed.properties.len(), 1);
    let property = &observed.properties[0];
    let V::Known(ty) = analysis
        .describe_body_type(body.id, property.context, property.value_type)
        .unwrap()
    else {
        panic!("member default context")
    };
    assert_eq!(ty.name, "T");
    let V::Known(selectors) = analysis.selectors(body.id, || false).unwrap() else {
        panic!("complete member selector")
    };
    assert_eq!(selectors.len(), 1);
    assert_eq!(selectors[0].selector, location(text, "item):", 4).span);
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let target = index
        .targets_at(&location(text, "item: T", 4))
        .iter()
        .copied()
        .find(|target| matches!(target, Target::Property(_)))
        .unwrap();
    assert_eq!(index.references_to(target).count(), 2);
}

#[test]
fn defaults_register_nested_type_and_accessor_regions_after_their_parent_completes() {
    let text = "fun never(seed: Boolean, value: Boolean = if (true) { class Local(val stored: Boolean) { val read: Boolean get() = stored }; val item = Local(seed); item.read } else { seed }): Boolean = value";
    let mut analysis = analyze_defaults(text);
    let body = region(&mut analysis, text, "Local(seed)");
    let observed = facts(&mut analysis, &body);
    assert!(
        observed
            .types
            .iter()
            .any(|reference| reference.origin.span == location(text, "Local(val", 5).span)
    );
    assert!(
        observed
            .callables
            .iter()
            .any(|reference| reference.span == location(text, "Local(seed)", 5).span)
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let usage = location(text, "stored };", 6);
    assert!(matches!(index.targets_at(&usage), [Target::Property(_)]));
}

#[test]
fn vararg_defaults_keep_the_declared_array_context_and_prior_parameter_reference() {
    let text = "fun never(seed: Boolean, vararg rest: Boolean = [seed]): Boolean = seed";
    let mut analysis = analyze_defaults(text);
    let body = region(&mut analysis, text, "[seed]");
    let observed = facts(&mut analysis, &body);
    let array = observed
        .expressions
        .iter()
        .find(|expression| expression.span == body.origin.span)
        .unwrap();
    let V::Known(ty) = analysis
        .describe_body_type(body.id, array.context, array.value_type)
        .unwrap()
    else {
        panic!("array default context")
    };
    assert_eq!(ty.name, "Array<Boolean>");
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let target = index.targets_at(&location(text, "seed: Boolean", 4))[0];
    assert_eq!(index.references_to(target).count(), 3);
}
