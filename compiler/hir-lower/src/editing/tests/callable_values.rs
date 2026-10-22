use super::*;

#[test]
fn callable_values_keep_selected_source_declarations_and_name_ranges() {
    for (text, name) in [
        (
            "fun choose(value: Boolean): Boolean = value\nfun choose(value: String): String = value\nfun editing(): (Boolean) -> Boolean = ::choose",
            "choose",
        ),
        (
            "fun <Item> pass(value: Item): Item = value\nfun editing(): (Boolean) -> Boolean = ::pass",
            "pass",
        ),
        (
            "class Value { fun member(): Boolean = true }\nfun editing(value: Value): () -> Boolean = value::member",
            "member",
        ),
        (
            "class Value { fun member(): Boolean = true }\nfun editing(): (Value) -> Boolean = Value::member",
            "member",
        ),
        (
            "fun Boolean.extension(): Boolean = this\nfun editing(value: Boolean): () -> Boolean = value::extension",
            "extension",
        ),
        (
            "fun Boolean.extension(): Boolean = this\nfun editing(): (Boolean) -> Boolean = Boolean::extension",
            "extension",
        ),
        (
            "fun editing(value: Boolean): () -> Boolean { fun local(): Boolean = value\nreturn ::local }",
            "local",
        ),
        (
            "@NoGC fun native(value: Int): Int = value\nfun editing(): FunPtr<(Int) -> Int> = ::native",
            "native",
        ),
    ] {
        let mut analysis = analyze(text);
        let editing = function(&analysis, "editing");
        let V::Known(facts) = analysis.function_facts(editing, || false).unwrap() else {
            panic!("{text}: {:?}", analysis.diagnostics());
        };
        assert_eq!(facts.callables.len(), 1, "{text}: {:?}", facts.callables);
        let reference = &facts.callables[0];
        assert_eq!(reference.span.start as usize, text.rfind(name).unwrap());
        assert_eq!(
            reference.span.end as usize,
            text.rfind(name).unwrap() + name.len()
        );
        assert_eq!(
            reference.definition.as_ref().unwrap().span.start as usize,
            text.find(name).unwrap()
        );
        assert!(matches!(
            reference.target.callable(),
            Some(hir::SemanticCallable::Current(_))
        ));
    }
}

#[test]
fn ambiguous_callable_values_do_not_publish_a_selected_reference() {
    let mut analysis = analyze(
        "fun choose(value: Boolean): Boolean = value\nfun choose(value: String): String = value\nfun editing() { val callback = ::choose }",
    );
    let function = function(&analysis, "editing");
    assert!(matches!(
        analysis.function_facts(function, || false).unwrap(),
        V::Unavailable(U::BodyFailure)
    ));
    assert!(
        analysis
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("ambiguous"))
    );
}
