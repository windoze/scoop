use super::*;

fn facts(analysis: &mut DeclarationAnalysis, name: &str) -> hir::EditingBodyFacts {
    let function = function(analysis, name);
    let V::Known(facts) = analysis.function_facts(function, || false).unwrap() else {
        panic!(
            "body facts for {name} are unavailable: {:?}",
            analysis.diagnostics()
        );
    };
    facts.clone()
}

fn describe(analysis: &mut DeclarationAnalysis, local: &hir::EditingLocalOccurrence) -> String {
    let function = function(analysis, "editing");
    let V::Known(description) = analysis
        .describe_body_type(function, local.context, local.value_type)
        .unwrap()
    else {
        panic!("a checked occurrence has its type context");
    };
    description.name
}

#[test]
fn declarations_reads_and_writes_keep_the_selected_binding() {
    let text =
        "fun editing(value: Boolean): Boolean { var copy = value\ncopy = false\nreturn copy }";
    let mut analysis = analyze(text);
    let body = facts(&mut analysis, "editing");
    assert_eq!(body.locals.len(), 5);
    let copies = body
        .locals
        .iter()
        .filter(|local| local.reference.name == "copy")
        .collect::<Vec<_>>();
    assert_eq!(copies.len(), 3);
    assert_eq!(copies[0].role, hir::EditingLocalRole::Declaration);
    assert_eq!(copies[1].role, hir::EditingLocalRole::Write);
    assert_eq!(copies[2].role, hir::EditingLocalRole::Read);
    for local in copies {
        assert_eq!(
            local.reference.definition.span.start as usize,
            text.find("copy").unwrap()
        );
        assert_eq!(describe(&mut analysis, local), "Boolean");
    }
    let literal = body
        .expressions
        .iter()
        .find(|expression| expression.span.start as usize == text.find("false").unwrap())
        .unwrap();
    assert!(matches!(literal.expected_type, V::Known(Some(ty)) if ty == literal.value_type));
}

#[test]
fn block_shadowing_and_capture_reads_keep_the_original_definitions() {
    let text = "fun editing(value: Boolean): Boolean { if (value) { val value = \"inner\"\nval copy = value }\nval read: () -> Boolean = { value }\nreturn read() }";
    let mut analysis = analyze(text);
    let body = facts(&mut analysis, "editing");
    let reads = body
        .locals
        .iter()
        .filter(|local| {
            local.reference.name == "value" && local.role == hir::EditingLocalRole::Read
        })
        .collect::<Vec<_>>();
    assert_eq!(reads.len(), 3);
    assert_eq!(describe(&mut analysis, reads[0]), "Boolean");
    assert_eq!(describe(&mut analysis, reads[1]), "String");
    assert_eq!(describe(&mut analysis, reads[2]), "Boolean");
    assert_eq!(reads[0].reference.binding, reads[2].reference.binding);
    assert_ne!(reads[0].reference.binding, reads[1].reference.binding);
    assert_ne!(reads[0].context, reads[2].context);
}

#[test]
fn nested_generic_function_types_use_their_own_binder() {
    let text =
        "fun <T> editing(value: T): T { fun <U> inner(item: U): U = item\nreturn inner(value) }";
    let mut analysis = analyze(text);
    let body = facts(&mut analysis, "editing");
    let inner = body
        .locals
        .iter()
        .find(|local| local.reference.name == "item" && local.role == hir::EditingLocalRole::Read)
        .unwrap();
    assert_eq!(describe(&mut analysis, inner), "U");
    let outer = body
        .locals
        .iter()
        .find(|local| local.reference.name == "value" && local.role == hir::EditingLocalRole::Read)
        .unwrap();
    assert_eq!(describe(&mut analysis, outer), "T");
}

#[test]
fn failed_body_and_previously_failed_inference_do_not_publish_partial_facts() {
    let mut analysis = analyze(
        "fun broken() = missing\nfun dependent(): Boolean = broken()\nfun healthy() = true",
    );
    for name in ["broken", "dependent"] {
        let function = function(&analysis, name);
        assert!(matches!(
            analysis.function_facts(function, || false).unwrap(),
            V::Unavailable(U::BodyFailure)
        ));
    }
    assert!(!facts(&mut analysis, "healthy").expressions.is_empty());
}

#[test]
fn expression_cancellation_discards_the_query_and_retry_observes_a_clean_body() {
    let mut analysis =
        analyze("fun editing(value: Boolean): Boolean { val copy = value\nreturn copy }");
    let function = function(&analysis, "editing");
    let checks = std::rc::Rc::new(std::cell::Cell::new(0));
    let observed = checks.clone();
    let result = analysis.function_facts(function, move || {
        observed.set(observed.get() + 1);
        observed.get() >= 4
    });
    assert!(matches!(result, Err(EditingCancelled)));
    assert!(checks.get() >= 4);
    assert!(!analysis.body_facts.contains_key(&function.into()));
    let body = facts(&mut analysis, "editing");
    assert_eq!(body.locals.len(), 4);
    assert!(analysis.diagnostics().is_empty());
}
