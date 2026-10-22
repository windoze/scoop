use super::*;

fn calls(analysis: &mut DeclarationAnalysis) -> Vec<hir::EditingAppliedCall> {
    let function = function(analysis, "editing");
    let V::Known(calls) = analysis.calls(function, || false).unwrap() else {
        panic!("available body input");
    };
    calls.to_vec()
}

fn type_name(
    analysis: &mut DeclarationAnalysis,
    call: &hir::EditingAppliedCall,
    ty: hir::TypeId,
) -> String {
    let function = function(analysis, "editing");
    let V::Known(description) = analysis
        .describe_body_type(function, call.context, ty)
        .unwrap()
    else {
        panic!("call keeps its actual binder context");
    };
    description.name
}

#[test]
fn applied_calls_keep_selected_types_and_named_source_order() {
    let text = "fun choose(first: Boolean, second: String): String = second\nfun choose(first: String, second: Boolean): String = first\nfun editing(): String = choose(second = \"yes\", first = true)";
    let mut analysis = analyze(text);
    let calls = calls(&mut analysis);
    assert_eq!(calls.len(), 1, "{:?}", analysis.diagnostics());
    let call = &calls[0];
    assert_eq!(call.name.as_deref(), Some("choose"));
    assert_eq!(
        call.arguments
            .iter()
            .map(|argument| argument.parameter)
            .collect::<Vec<_>>(),
        [1, 0]
    );
    assert_eq!(
        type_name(&mut analysis, call, call.parameters[0].value_type),
        "Boolean"
    );
    assert_eq!(type_name(&mut analysis, call, call.result), "String");
    assert_eq!(
        &text[call.arguments[0].span.start as usize..call.arguments[0].span.end as usize],
        "second = \"yes\""
    );
}

#[test]
fn generic_varargs_keep_element_spread_and_named_array_bindings() {
    let text = "fun <T> gather(value: T, vararg rest: T): T = value\nfun editing(rest: Array<Boolean>): Boolean { gather(true, false, true)\ngather(true, *rest)\nreturn gather(rest = rest, value = true) }";
    let mut analysis = analyze(text);
    let calls = calls(&mut analysis);
    assert_eq!(calls.len(), 3, "{:?}", analysis.diagnostics());
    for (call, expected) in calls.iter().zip([vec![0, 1, 1], vec![0, 1], vec![1, 0]]) {
        assert_eq!(
            call.arguments
                .iter()
                .map(|argument| argument.parameter)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            type_name(
                &mut analysis,
                call,
                call.parameters[1].vararg_element_type.unwrap()
            ),
            "Boolean"
        );
    }
}

#[test]
fn omitted_defaults_and_trailing_lambdas_keep_their_distinct_inputs() {
    let mut analysis = analyze(
        "fun accept(flag: Boolean = true, task: () -> Boolean): Boolean = task()\nfun editing(): Boolean = accept() { true }",
    );
    let calls = calls(&mut analysis);
    assert_eq!(calls.len(), 1, "{:?}", analysis.diagnostics());
    let call = &calls[0];
    assert_eq!(call.parameters.len(), 2);
    assert_eq!(call.arguments.len(), 1);
    assert_eq!(call.arguments[0].parameter, 1);
    assert!(call.arguments[0].trailing);
}

#[test]
fn function_values_keep_unnamed_parameters_without_a_synthetic_invoke_name() {
    for text in [
        "fun editing(action: (Boolean) -> String): String = action(true)",
        "fun editing(action: (Boolean) -> String): String = (action)(true)",
    ] {
        let mut analysis = analyze(text);
        let calls = calls(&mut analysis);
        assert_eq!(calls.len(), 1, "{text}: {:?}", analysis.diagnostics());
        let call = &calls[0];
        assert_ne!(call.name.as_deref(), Some("invoke"));
        assert!(call.parameters[0].name.is_none());
        assert_eq!(call.arguments[0].parameter, 0);
        assert_eq!(
            type_name(&mut analysis, call, call.parameters[0].value_type),
            "Boolean"
        );
        assert_eq!(type_name(&mut analysis, call, call.result), "String");
    }
}

#[test]
fn methods_constructors_and_extensions_share_applied_argument_facts() {
    for (text, name) in [
        (
            "class Value { fun member(flag: Boolean): Boolean = flag }\nfun editing(value: Value): Boolean = value.member(true)",
            "member",
        ),
        (
            "fun Boolean.extension(flag: Boolean): Boolean = flag\nfun editing(value: Boolean): Boolean = value.extension(true)",
            "extension",
        ),
        (
            "struct Value(val flag: Boolean)\nfun editing(): Value = Value(true)",
            "Value",
        ),
    ] {
        let mut analysis = analyze(text);
        let calls = calls(&mut analysis);
        assert_eq!(calls.len(), 1, "{text}: {:?}", analysis.diagnostics());
        let call = &calls[0];
        assert_eq!(call.name.as_deref(), Some(name));
        assert_eq!(call.arguments[0].parameter, 0);
        assert_eq!(
            type_name(&mut analysis, call, call.parameters[0].value_type),
            "Boolean"
        );
    }
}

#[test]
fn earlier_checked_calls_survive_body_failure_but_failed_probes_do_not() {
    let mut analysis = analyze(
        "fun accept(value: Boolean) {}\nfun editing() { accept(true)\naccept(\"bad\")\naccept(false) }",
    );
    let calls = calls(&mut analysis);
    assert_eq!(calls.len(), 1);
    let function = function(&analysis, "editing");
    assert!(matches!(
        analysis.function_facts(function, || false).unwrap(),
        V::Unavailable(U::BodyFailure)
    ));
    assert_eq!(
        type_name(&mut analysis, &calls[0], calls[0].parameters[0].value_type),
        "Boolean"
    );
}
