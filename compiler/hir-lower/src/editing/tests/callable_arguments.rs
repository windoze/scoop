use super::receivers::{known_type, receiver};
use super::*;

#[test]
fn a_unique_call_supplies_explicit_and_implicit_lambda_parameters() {
    for expression in [
        "use({ value -> value. })",
        "use({ it. })",
        "use(block = { value -> value. })",
        "use(fun(value: String): Boolean { value. })",
    ] {
        let text = format!(
            "fun use(block: (String) -> Boolean) {{}}\nfun editing(value: Boolean, it: Boolean) {{ {expression} }}"
        );
        let mut analysis = analyze(&text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
        assert!(
            facts.local.as_ref().unwrap().definition.span.start as usize
                > text.find("{ use").unwrap()
        );
    }
}

#[test]
fn supplied_inputs_infer_parameters_even_when_the_callable_result_is_pending() {
    for expression in [
        "map(\"known\", { value -> value. })",
        "map(seed = \"known\", block = { value -> value. })",
        "map<String, _>(\"known\", { value -> value. })",
        "map(\"known\", fun(value: String) { value. })",
    ] {
        let text = format!(
            "fun <T, R> map(seed: T, block: (T) -> R): R = block(seed)\nfun editing() {{ {expression} }}"
        );
        let mut analysis = analyze(&text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
    }
}

#[test]
fn named_arguments_use_the_actual_parameter_mapping_and_source_order() {
    let text = "fun use(block: (String) -> Boolean, flag: Boolean) {}\nfun editing() { use(flag = true, block = { 值 -> 值. }) }";
    let mut analysis = analyze(text);
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "String");
    assert_eq!(
        facts.local.as_ref().unwrap().definition.span.start as usize,
        text.find("值 ->").unwrap()
    );
    let id = function(&analysis, "editing");
    let calls = analysis.incomplete_calls(id, || false).unwrap();
    let V::Known(candidates) = &calls[0].candidates else {
        panic!("known call")
    };
    assert_eq!(candidates[0].source_parameters, [1, 0]);
    assert_eq!(candidates[0].unchecked_arguments, [1]);
}

#[test]
fn receiver_and_destructuring_contexts_reuse_the_inferred_callable_signature() {
    for (declaration, expression) in [
        (
            "fun <T, R> use(seed: T, block: T.() -> R): R = seed.block()",
            "use(\"known\", { this. })",
        ),
        (
            "fun <T, R> use(seed: T, block: T.() -> R): R = seed.block()",
            "use(\"known\", fun String.() { this. })",
        ),
        (
            "fun use(block: ((String, Boolean)) -> Boolean) {}",
            "use({ (value, flag) -> value. })",
        ),
        (
            "struct Box<T>(val value: T)\nfun <T, R> use(seed: T, block: (Box<T>) -> R) {}",
            "use(\"known\", { Box(value) -> value. })",
        ),
    ] {
        let text = format!("{declaration}\nfun editing() {{ {expression} }}");
        let mut analysis = analyze(&text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
    }
}

#[test]
fn candidate_local_nominal_types_remain_in_the_arena_used_for_body_queries() {
    let text = "struct Box<T>(val value: T)\nfun <T, R> use(seed: T, block: (Box<T>) -> R) {}\nfun editing() { use(\"known\", { value -> value. }) }";
    let mut analysis = analyze(text);
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "Box<String>");
    let V::Known(members) = &facts.members else {
        panic!("instantiated members")
    };
    assert!(
        members
            .candidates
            .iter()
            .any(|member| member.name == "value")
    );
}

#[test]
fn members_extensions_constructors_and_function_values_share_argument_contexts() {
    for text in [
        "class Host { fun use(block: (String) -> Boolean) {} }\nfun editing(host: Host) { host.use({ value -> value. }) }",
        "fun Boolean.use(block: (String) -> Boolean) {}\nfun editing() { true.use({ value -> value. }) }",
        "struct Host<T>(val seed: T, val block: (T) -> Boolean)\nfun editing() { Host(\"known\", { value -> value. }) }",
        "fun editing(action: ((String) -> Boolean) -> Boolean) { (action)({ value -> value. }) }",
        "fun editing(action: ((String) -> Boolean) -> Boolean) { action({ value -> value. }) }",
        "class Host(val action: ((String) -> Boolean) -> Boolean)\nfun editing(host: Host) { host.action({ value -> value. }) }",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
    }
}

#[test]
fn the_available_prefix_filters_overloads_before_entering_the_literal() {
    let text = "fun use(seed: String, block: (String) -> Boolean) {}\nfun use(seed: Boolean, block: (Boolean) -> Boolean) {}\nfun editing() { use(\"known\", { value -> value. }) }";
    let mut analysis = analyze(text);
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "String");
    let id = function(&analysis, "editing");
    let calls = analysis.incomplete_calls(id, || false).unwrap();
    assert!(matches!(&calls[0].candidates, V::Known(candidates) if candidates.len() == 1));
}

#[test]
fn nested_argument_bodies_can_filter_their_own_incomplete_calls() {
    let text = "fun use(block: (String) -> Boolean) {}\nfun choose(seed: String, flag: Boolean) {}\nfun choose(seed: Boolean, flag: Boolean) {}\nfun editing() { use({ value -> choose(value, }) }";
    let mut analysis = analyze(text);
    let id = function(&analysis, "editing");
    let calls = analysis.incomplete_calls(id, || false).unwrap();
    assert_eq!(calls.len(), 2);
    let inner = calls
        .iter()
        .find(|call| call.name.as_deref() == Some("choose"))
        .unwrap();
    let V::Known(candidates) = &inner.candidates else {
        panic!("context inside the literal: {inner:?}")
    };
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].parameters[0].type_label, "String");
}
