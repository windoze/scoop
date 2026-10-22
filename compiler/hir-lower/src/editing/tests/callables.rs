use super::*;

fn references(analysis: &mut DeclarationAnalysis) -> Vec<hir::EditingCallableReference> {
    let function = function(analysis, "editing");
    let V::Known(facts) = analysis.function_facts(function, || false).unwrap() else {
        panic!("call body failed: {:?}", analysis.diagnostics());
    };
    facts.callables.clone()
}

#[test]
fn overload_navigation_uses_the_selected_function_declaration() {
    let text = "fun choose(value: Boolean): Boolean = value\nfun choose(value: String): Boolean = true\nfun editing(): Boolean = choose(true)";
    let mut analysis = analyze(text);
    let references = references(&mut analysis);
    assert_eq!(references.len(), 1);
    let reference = &references[0];
    assert_eq!(reference.span.start as usize, text.rfind("choose").unwrap());
    assert_eq!(
        reference.definition.as_ref().unwrap().span.start as usize,
        text.find("choose").unwrap()
    );
    let hir::SemanticCallable::Current(target) = reference.target else {
        panic!("source function")
    };
    let V::Known(signature) = analysis.signature(target) else {
        panic!("checked target")
    };
    let V::Known(ty) = analysis
        .describe_type(target, signature.parameters[0].value_type)
        .unwrap()
    else {
        panic!("checked parameter")
    };
    assert_eq!(ty.name, "Boolean");
}

#[test]
fn different_generic_applications_navigate_to_one_source_declaration() {
    let text = "fun <T> identity(value: T): T = value\nfun editing(): Boolean { val text = identity(\"value\")\nreturn identity(true) }";
    let mut analysis = analyze(text);
    let references = references(&mut analysis);
    assert_eq!(references.len(), 2);
    assert_eq!(references[0].target, references[1].target);
    assert_eq!(
        references[0].definition.as_ref().unwrap().span,
        references[1].definition.as_ref().unwrap().span
    );
}

#[test]
fn local_methods_and_extensions_keep_actual_name_tokens() {
    for (text, name) in [
        (
            "class Value { fun member(): Boolean = true }\nfun editing(value: Value): Boolean = value.member()",
            "member",
        ),
        (
            "fun Boolean.extension(): Boolean = this\nfun editing(value: Boolean): Boolean = value.extension()",
            "extension",
        ),
        (
            "fun editing(): Boolean { fun local(): Boolean = true\nreturn local() }",
            "local",
        ),
    ] {
        let mut analysis = analyze(text);
        let references = references(&mut analysis);
        assert_eq!(references.len(), 1, "{text}: {references:?}");
        let reference = &references[0];
        assert_eq!(reference.span.start as usize, text.rfind(name).unwrap());
        assert_eq!(
            reference.definition.as_ref().unwrap().span.start as usize,
            text.find(name).unwrap()
        );
    }
}

#[test]
fn function_value_and_implicit_invoke_names_remain_local_references() {
    for text in [
        "fun editing(value: () -> Boolean): Boolean = value()",
        "class Value { operator fun invoke(): Boolean = true }\nfun editing(value: Value): Boolean = value()",
    ] {
        let mut analysis = analyze(text);
        assert!(references(&mut analysis).is_empty(), "{text}");
        let function = function(&analysis, "editing");
        let V::Known(facts) = analysis.function_facts(function, || false).unwrap() else {
            panic!("checked invocation")
        };
        assert!(facts.locals.iter().any(|local| local.span.start as usize
            == text.rfind("value").unwrap()
            && local.role == hir::EditingLocalRole::Read));
    }
}
