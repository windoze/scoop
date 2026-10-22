use super::*;

fn selectors(analysis: &mut DeclarationAnalysis) -> Vec<hir::EditingSelector> {
    let function = function(analysis, "editing");
    let V::Known(selectors) = analysis.selectors(function, || false).unwrap() else {
        panic!("body input is available");
    };
    selectors.to_vec()
}

#[test]
fn a_failed_selector_keeps_its_checked_receiver_without_a_successful_body() {
    let text =
        "class Value { fun member(): Boolean = true }\nfun editing(value: Value) { value.mem }";
    let mut analysis = analyze(text);
    let selectors = selectors(&mut analysis);
    assert_eq!(selectors.len(), 1);
    let selector = &selectors[0];
    assert_eq!(
        &text[selector.selector.start as usize..selector.selector.end as usize],
        "mem"
    );
    let V::Known(members) = &selector.members else {
        panic!("known receiver");
    };
    assert!(
        members
            .candidates
            .iter()
            .any(|member| member.name == "member")
    );
    let function = function(&analysis, "editing");
    assert!(matches!(
        analysis.function_facts(function, || false).unwrap(),
        V::Unavailable(U::BodyFailure)
    ));
    assert_eq!(
        selector.local.as_ref().unwrap().definition.span.start as usize,
        text.find("value:").unwrap()
    );
    let V::Known(description) = analysis
        .describe_body_type(function, selector.context, selector.value_type)
        .unwrap()
    else {
        panic!("real receiver context");
    };
    assert_eq!(description.name, "Value");
}

#[test]
fn nested_lambda_selector_uses_the_shadowing_parameter() {
    let text = "fun Boolean.onlyBoolean(): Boolean = this\nfun String.onlyString(): String = this\nfun editing(value: String) { val closure: (Boolean) -> Boolean = { value -> value.partial() } }";
    let mut analysis = analyze(text);
    let selectors = selectors(&mut analysis);
    assert_eq!(selectors.len(), 1, "{selectors:?}");
    let V::Known(members) = &selectors[0].members else {
        panic!("lambda receiver");
    };
    assert!(
        members
            .candidates
            .iter()
            .any(|member| member.name == "onlyBoolean")
    );
    assert!(
        !members
            .candidates
            .iter()
            .any(|member| member.name == "onlyString")
    );
    assert_eq!(
        selectors[0].local.as_ref().unwrap().definition.span.start as usize,
        text.find("value ->").unwrap()
    );
}

#[test]
fn selector_candidates_use_the_actual_smart_cast() {
    let text = "fun String.stringMember(): Boolean = true\nfun editing(value: Any) { if (value is String) { value.partial() } }";
    let mut analysis = analyze(text);
    let selectors = selectors(&mut analysis);
    assert_eq!(selectors.len(), 1, "{selectors:?}");
    let V::Known(members) = &selectors[0].members else {
        panic!("narrowed receiver");
    };
    assert!(
        members
            .candidates
            .iter()
            .any(|member| member.name == "stringMember")
    );
}

#[test]
fn an_earlier_body_failure_does_not_publish_later_selector_facts() {
    let mut analysis =
        analyze("fun editing(value: Boolean) { val broken = missing\nvalue.partial() }");
    assert!(selectors(&mut analysis).is_empty());
}

#[test]
fn rejected_lambda_overloads_do_not_publish_speculative_receiver_types() {
    let mut analysis = analyze(
        "fun choose(block: (Boolean) -> Boolean) {}\nfun choose(block: (String) -> String) {}\nfun editing() { choose { value -> value.partial() } }",
    );
    assert!(selectors(&mut analysis).is_empty());
}
