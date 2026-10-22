use super::*;
use crate::Owner;

#[test]
fn recovered_generic_enum_variants_defaults_and_conformance_share_the_original_types() {
    let text = "interface Contract<T> { fun editing(value: T): T }\nenum Choice<T> : Contract<T> {\nEmpty, Full(val item: T, val enabled: Boolean = true)\npublic override fun editing(value: T): T { value. }\nfun members() { this. }\nfun healthy(value: T): T = value\n}\nfun consumer(value: Choice<Boolean>): Boolean = value.healthy(true)\nfun construct(): Choice<Boolean> = Choice.Full(true)";
    let mut analysis = analyze(text);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    let healthy = function(&analysis, "healthy");
    let Owner::Enum(owner) = analysis.lowerer.function_owner[&healthy] else {
        panic!("enum method owner")
    };
    assert_eq!(
        analysis.lowerer.enums[owner]
            .variants
            .iter()
            .map(|variant| variant.name.as_str())
            .collect::<Vec<_>>(),
        ["Empty", "Full"]
    );
    let editing = analysis.lowerer.enums[owner]
        .methods
        .iter()
        .copied()
        .find(|method| analysis.lowerer.source_function_name(*method) == "editing")
        .unwrap();
    let receiver = analysis.receivers(editing, || false).unwrap()[0].clone();
    let V::Known(ty) = receiver.value_type else {
        panic!("known enum receiver")
    };
    let V::Known(description) = analysis.describe_type(editing, ty).unwrap() else {
        panic!("enum parameter context");
    };
    assert_eq!(description.name, "T");
    for name in ["healthy", "consumer", "construct"] {
        let function = function(&analysis, name);
        assert!(
            matches!(
                analysis.function_facts(function, || false).unwrap(),
                V::Known(_)
            ),
            "{name}: {:?}",
            analysis.diagnostics()
        );
    }
}

#[test]
fn recovered_integral_enum_keeps_all_values_for_exhaustiveness() {
    let text = "enum Flag : Int {\nOff = 1, On = 2\nfun editing() { this. }\n}\nfun complete(value: Flag): Boolean = when (value) { case Flag.Off -> false; case Flag.On -> true }\nfun incomplete(value: Flag): Boolean = when (value) { case Flag.Off -> false }";
    let mut analysis = analyze(text);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    let editing = function(&analysis, "editing");
    let Owner::Enum(owner) = analysis.lowerer.function_owner[&editing] else {
        panic!("enum owner")
    };
    assert!(
        analysis.lowerer.enums[owner]
            .representation
            .integer_kind()
            .is_some()
    );
    assert_eq!(analysis.lowerer.enums[owner].variants.len(), 2);
    let complete = function(&analysis, "complete");
    assert!(matches!(
        analysis.function_facts(complete, || false).unwrap(),
        V::Known(_)
    ));
    let incomplete = function(&analysis, "incomplete");
    assert!(matches!(
        analysis.function_facts(incomplete, || false).unwrap(),
        V::Unavailable(U::BodyFailure)
    ));
}

#[test]
fn recovered_nested_enum_preserves_its_own_generic_context() {
    for text in [
        "class Outer<T> { enum Inner<U> { Value(U)\nfun editing(value: U) { value. } } }",
        "enum Outer<T> { Empty\nstruct Inner<U>(val field: U) { fun editing(value: U) { value. } } }",
    ] {
        let mut analysis = analyze(text);
        assert!(
            analysis.diagnostics().is_empty(),
            "{:?}",
            analysis.diagnostics()
        );
        let receiver = receivers::receiver(&mut analysis);
        assert_eq!(receivers::known_type(&mut analysis, &receiver), "U");
    }
}

#[test]
fn recovered_integral_methods_still_obey_the_original_operator_rule() {
    let analysis =
        analyze("enum Flag : Int { Off = 1\noperator fun plus(value: Int): Int { value. } }");
    assert!(
        analysis
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message
                == "integral enums use the integer parent's operators"),
        "{:?}",
        analysis.diagnostics()
    );
}
