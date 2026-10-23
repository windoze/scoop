use super::*;
use candidates::call_candidates;

#[test]
fn qualified_variant_aliases_keep_their_own_parameters_and_fixed_applications() {
    for (alias, call, expected) in [
        (
            "typealias Alias<U> = Choice<U>",
            "Alias.Full<Boolean>(",
            "U",
        ),
        (
            "typealias Alias = Choice<Boolean>",
            "Alias.Full(",
            "Boolean",
        ),
        (
            "typealias Alias<U> = Choice<U>",
            "Alias<Boolean>.Full(",
            "Boolean",
        ),
    ] {
        let text = format!(
            "enum Choice<T> {{ Full(val item: T, val flag: Boolean = true) }}\n{alias}\nfun editing() {{ {call}"
        );
        let candidates = call_candidates(&text);
        assert_eq!(candidates.len(), 1, "{text}: {candidates:?}");
        assert_eq!(candidates[0].parameters[0].type_label, expected, "{text}");
        assert!(candidates[0].parameters[1].has_default);
    }
    let candidates = call_candidates(
        "enum Choice<T> { Full(T, Boolean) }\ntypealias Alias<U> = Choice<U>\nfun editing() { Alias.Full<Boolean>(\"wrong\",",
    );
    assert!(candidates.is_empty(), "{candidates:?}");
}

#[test]
fn qualified_aliases_keep_nested_bindings_and_lexical_value_shadowing() {
    for (text, expected) in [
        (
            "struct Value<T>(val item: T)\nclass Host { typealias Make<Item> = Value<Item> }\nfun editing() { Host.Make<Boolean>(",
            "Item",
        ),
        (
            "enum Choice<T> { Full(T) }\ntypealias Alias = Choice<Boolean>\nclass Maker { fun Full(text: String): String = text }\nfun editing(Alias: Maker) { Alias.Full(",
            "String",
        ),
    ] {
        let candidates = call_candidates(text);
        assert_eq!(candidates.len(), 1, "{text}: {candidates:?}");
        assert_eq!(candidates[0].parameters[0].type_label, expected, "{text}");
    }
}

#[test]
fn type_parameter_companions_use_the_original_bound_receiver() {
    let candidates = call_candidates(
        "interface Convert<Item> { fun convert(value: Item): Item }\nfun <T> editing() where T.Companion : Convert<T> { T.convert(",
    );
    assert_eq!(candidates.len(), 1, "{candidates:?}");
    assert_eq!(candidates[0].parameters[0].type_label, "T");
    assert!(matches!(&candidates[0].result, V::Known(label) if label == "T"));
}
