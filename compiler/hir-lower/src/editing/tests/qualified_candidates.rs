use super::*;
use candidates::call_candidates;

#[test]
fn qualified_variants_keep_the_original_generic_signature_and_defaults() {
    for (call, parameter, result) in [
        ("Choice.Full<Boolean>(", "T", "Choice<T>"),
        ("Choice<Boolean>.Full(", "Boolean", "Choice<Boolean>"),
    ] {
        let text = format!(
            "enum Choice<T> {{ Empty, Full(val item: T, val enabled: Boolean = true) }}\nfun editing() {{ {call}"
        );
        let candidates = call_candidates(&text);
        assert_eq!(candidates.len(), 1, "{text}: {candidates:?}");
        let candidate = &candidates[0];
        assert_eq!(candidate.parameters.len(), 2);
        assert_eq!(candidate.parameters[0].type_label, parameter, "{text}");
        assert_eq!(candidate.parameters[1].type_label, "Boolean");
        assert!(candidate.parameters[1].has_default);
        assert!(
            matches!(&candidate.result, V::Known(label) if label == result),
            "{text}: {candidate:?}"
        );
    }
    let candidates = call_candidates(
        "enum Choice<T> { Full(T) }\nfun editing() { Choice<Boolean>.Full(\"wrong\",",
    );
    assert!(candidates.is_empty(), "{candidates:?}");
}

#[test]
fn qualified_nested_constructors_and_companions_keep_their_targets() {
    for text in [
        "class Outer { struct Nested(val flag: Boolean) }\nfun editing() { Outer.Nested(",
        "class Outer { class Nested(val flag: Boolean) }\nfun editing() { Outer.Nested(",
        "class Outer { companion object { fun make(flag: Boolean): Boolean = flag } }\nfun editing() { Outer.make(",
        "object Factory { fun make(flag: Boolean): Boolean = flag }\nfun editing() { Factory.make(",
    ] {
        let candidates = call_candidates(text);
        assert_eq!(candidates.len(), 1, "{text}: {candidates:?}");
        assert_eq!(candidates[0].parameters[0].type_label, "Boolean", "{text}");
    }
}

#[test]
fn local_values_shadow_qualified_types_for_direct_and_safe_calls() {
    for (parameter, call) in [
        ("Factory", "Choice.Full("),
        ("Option<Factory>", "Choice?.Full("),
    ] {
        let text = format!(
            "enum Choice {{ Full(Boolean) }}\nclass Factory {{ fun Full(text: String): String = text }}\nfun editing(Choice: {parameter}) {{ {call}"
        );
        let candidates = call_candidates(&text);
        assert_eq!(candidates.len(), 1, "{text}: {candidates:?}");
        assert_eq!(candidates[0].parameters[0].type_label, "String");
    }
}

#[test]
fn unit_variants_and_inaccessible_nested_types_do_not_offer_constructors() {
    assert!(call_candidates("enum Choice { Empty }\nfun editing() { Choice.Empty(").is_empty());
    let mut analysis = analyze(
        "class Outer { private class Nested(val flag: Boolean) }\nfun editing() { Outer.Nested(",
    );
    let function = function(&analysis, "editing");
    let calls = analysis.incomplete_calls(function, || false).unwrap();
    assert_eq!(calls.len(), 1);
    assert!(
        matches!(calls[0].candidates, V::Unavailable(_)),
        "{calls:?}"
    );
}
