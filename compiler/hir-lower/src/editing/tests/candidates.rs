use super::*;

fn call_candidates(text: &str) -> Vec<hir::EditingCallCandidate> {
    let mut analysis = analyze(text);
    let function = function(&analysis, "editing");
    let calls = analysis.incomplete_calls(function, || false).unwrap();
    assert_eq!(calls.len(), 1, "{text}: {calls:?}");
    let V::Known(candidates) = &calls[0].candidates else {
        panic!("{text}: {:?}", calls[0]);
    };
    candidates.clone()
}

#[test]
fn supplied_arguments_filter_overloads_without_finishing_the_call() {
    let text = "fun choose(flag: Boolean, text: String): String = text\nfun choose(flag: String, text: String): String = text\nfun editing() { choose(true,";
    let candidates = call_candidates(text);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].parameters[0].type_label, "Boolean");
    assert_eq!(candidates[0].source_parameters, [0, 1]);
    assert!(candidates[0].missing_parameters.is_empty());
    assert_eq!(candidates[0].unchecked_arguments, [1]);
}

#[test]
fn an_empty_unclosed_call_keeps_all_possible_declarations() {
    let candidates = call_candidates(
        "fun choose(value: Boolean): Boolean = value\nfun choose(value: String): String = value\nfun editing() { choose(",
    );
    assert_eq!(candidates.len(), 2);
    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.missing_parameters == [0])
    );
}

#[test]
fn missing_slots_keep_names_positions_and_stop_later_argument_typing() {
    let candidates = call_candidates(
        "fun choose(first: Boolean, second: String, third: Boolean) {}\nfun editing() { choose(true, , \"not checked after unknown\") }",
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].source_parameters, [0, 1, 2]);
    assert_eq!(candidates[0].unchecked_arguments, [1, 2]);
    let candidates = call_candidates(
        "fun choose(first: Boolean, second: String) {}\nfun editing() { choose(second = , first = true) }",
    );
    assert_eq!(candidates[0].source_parameters, [1, 0]);
    assert_eq!(candidates[0].unchecked_arguments, [0, 1]);
}

#[test]
fn generic_candidates_show_declarations_in_their_own_binders() {
    let candidates = call_candidates(
        "fun <Item> choose(value: Item, vararg rest: Item): Item = value\nfun <Caller> editing() { choose(true,",
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].parameters[0].type_label, "Item");
    assert_eq!(candidates[0].parameters[1].type_label, "Item");
    assert!(candidates[0].parameters[1].vararg);
    assert!(matches!(&candidates[0].result, V::Known(name) if name == "Item"));
}

#[test]
fn defaults_are_described_without_checking_or_executing_them() {
    let candidates = call_candidates(
        "fun choose(flag: Boolean = missing, text: String) {}\nfun editing() { choose(text =",
    );
    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].parameters[0].has_default);
    assert_eq!(candidates[0].source_parameters, [1]);
    assert_eq!(candidates[0].missing_parameters, [0]);
}

#[test]
fn callable_local_values_shadow_named_functions_and_have_no_parameter_names() {
    let candidates = call_candidates(
        "fun action(value: String): String = value\nfun editing(action: (Boolean) -> Boolean) { action(",
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].parameters[0].type_label, "Boolean");
    assert!(candidates[0].parameters[0].name.is_none());
}

#[test]
fn prefix_local_functions_keep_captured_owner_parameters() {
    let candidates = call_candidates(
        "fun <Item> editing(value: Item) { fun local(arg: Item): Item = arg\nlocal(",
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].parameters[0].type_label, "Item");
}

#[test]
fn members_extensions_constructors_and_aliases_use_original_lookup() {
    for text in [
        "class Value { fun member(flag: Boolean): Boolean = flag }\nfun editing(value: Value) { value.member(",
        "fun Boolean.member(flag: Boolean): Boolean = flag\nfun editing(value: Boolean) { value.member(",
        "struct Value(val flag: Boolean)\nfun editing() { Value(",
        "struct Value<T>(val flag: T)\ntypealias Alias = Value<Boolean>\nfun editing() { Alias(",
        "fun editing(value: (Boolean) -> Boolean) { (value)(",
        "fun editing(value: (Boolean) -> Boolean) { value.invoke(",
    ] {
        let candidates = call_candidates(text);
        assert_eq!(candidates.len(), 1, "{text}: {candidates:?}");
        assert_eq!(candidates[0].parameters[0].type_label, "Boolean", "{text}");
    }
}

#[test]
fn an_earlier_unknown_statement_blocks_later_call_candidates() {
    let mut analysis =
        analyze("fun choose(flag: Boolean) {}\nfun editing() { val unknown =\nchoose( }");
    let function = function(&analysis, "editing");
    let calls = analysis.incomplete_calls(function, || false).unwrap();
    assert_eq!(calls.len(), 1);
    assert!(matches!(
        calls[0].candidates,
        V::Unavailable(U::IncompleteSyntax)
    ));
}

#[test]
fn cancellation_does_not_publish_partial_recovery_facts() {
    let mut analysis = analyze("fun choose(flag: Boolean) {}\nfun editing() { choose(");
    let function = function(&analysis, "editing");
    let checks = std::cell::Cell::new(0);
    let cancelled = analysis.incomplete_calls(function, || {
        checks.set(checks.get() + 1);
        checks.get() >= 3
    });
    assert!(matches!(cancelled, Err(EditingCancelled)));
    assert!(!analysis.recovered_facts.contains_key(&function));
    let calls = analysis.incomplete_calls(function, || false).unwrap();
    assert!(matches!(&calls[0].candidates, V::Known(candidates) if candidates.len() == 1));
}

#[test]
fn callable_properties_precede_same_named_extensions() {
    for text in [
        "class Host(val action: (Boolean) -> Boolean)\nfun Host.action(text: String): String = text\nfun editing(host: Host) { host.action(",
        "class Host(val action: (Boolean) -> Boolean)\nfun Host.action(text: String): String = text\nfun editing(host: Option<Host>) { host?.action(",
        "class Host(val action: (Boolean) -> Boolean)\nfun Host.action(text: String): String = text\nfun Host.editing() { action(",
    ] {
        let candidates = call_candidates(text);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].parameters[0].type_label, "Boolean");
        assert!(candidates[0].parameters[0].name.is_none());
    }
}

#[test]
fn receiver_function_values_bind_the_receiver_before_source_arguments() {
    let candidates = call_candidates(
        "class Host\nfun Host.action(text: String): String = text\nfun editing(host: Host, action: Host.(Boolean) -> Boolean) { host.action(",
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].parameters.len(), 1);
    assert_eq!(candidates[0].parameters[0].type_label, "Boolean");
    assert!(candidates[0].parameters[0].name.is_none());
}

#[test]
fn property_and_extension_invoke_partitions_preserve_lookup_order() {
    for (text, expected) in [
        (
            "class Action\nclass Host(val action: Action)\noperator fun Action.invoke(flag: Boolean): Boolean = flag\nfun Host.action(text: String): String = text\nfun editing(host: Host) { host.action(",
            "String",
        ),
        (
            "class Action\nclass Host(val action: Action)\noperator fun Action.invoke(flag: Boolean): Boolean = flag\nfun editing(host: Host) { host.action(",
            "Boolean",
        ),
        (
            "class Action\nclass Host(val action: Action)\noperator fun Action.invoke(flag: Boolean): Boolean = flag\nfun Host.editing() { action(",
            "Boolean",
        ),
        (
            "class Action\nval action: Action = Action()\noperator fun Action.invoke(flag: Boolean): Boolean = flag\nfun editing() { action(",
            "Boolean",
        ),
        (
            "class Action\nclass Host\nval Host.action: Action get() = Action()\noperator fun Action.invoke(flag: Boolean): Boolean = flag\nfun editing(host: Host) { host.action(",
            "Boolean",
        ),
        (
            "class Action\nclass Host\nval Host.action: Action get() = Action()\noperator fun Action.invoke(flag: Boolean): Boolean = flag\nfun Host.editing() { action(",
            "Boolean",
        ),
    ] {
        let candidates = call_candidates(text);
        assert_eq!(candidates.len(), 1, "{text}: {candidates:?}");
        assert_eq!(candidates[0].parameters[0].type_label, expected, "{text}");
    }
}
