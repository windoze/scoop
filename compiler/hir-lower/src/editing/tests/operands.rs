use super::candidates::call_candidates;
use super::receivers::{known_type, receiver};
use super::*;

#[test]
fn nested_blocks_keep_actual_prefix_bindings_and_stop_after_unknowns() {
    let text = "fun editing(value: Boolean) { { val value = \"inner\"; value. }; value. }";
    let mut analysis = analyze(text);
    let id = function(&analysis, "editing");
    let facts = analysis.receivers(id, || false).unwrap().to_vec();
    assert_eq!(facts.len(), 2);
    assert_eq!(known_type(&mut analysis, &facts[0]), "String");
    assert_eq!(
        facts[0].local.as_ref().unwrap().definition.span.start as usize,
        text.find("value =").unwrap()
    );
    assert!(matches!(
        facts[1].value_type,
        V::Unavailable(U::IncompleteSyntax)
    ));
    let mut analysis = analyze("fun editing(value: Boolean) { { val unknown = ; value. } }");
    assert!(matches!(
        receiver(&mut analysis).value_type,
        V::Unavailable(U::IncompleteSyntax)
    ));
}

#[test]
fn an_initializer_reads_the_previous_binding_and_checks_its_annotation() {
    for text in [
        "fun editing(value: Boolean) { val value = value. }",
        "fun editing(value: Boolean) { val value: String =\nvalue. }",
        "fun editing(value: Boolean) { { val copy: String = value. } }",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "Boolean");
        assert_eq!(
            facts.local.unwrap().definition.span.start as usize,
            text.find("value").unwrap()
        );
    }
    let mut analysis = analyze("fun editing(value: Boolean) { val copy: Missing = value. }");
    assert!(matches!(
        receiver(&mut analysis).value_type,
        V::Unavailable(U::BodyFailure)
    ));
}

#[test]
fn unfinished_call_operands_use_the_same_candidate_query() {
    for body in [
        "{ val value = choose(true,",
        "{ return choose(true,",
        "{ return@editing choose(true,",
        "{ throw choose(true,",
        "{ { val value = true; choose(value,",
    ] {
        let text = format!(
            "fun choose(value: Boolean, text: String): String = text\nfun choose(value: String, text: String): String = text\nfun editing(): String {body}"
        );
        let candidates = call_candidates(&text);
        assert_eq!(candidates.len(), 1, "{body}");
        assert_eq!(candidates[0].parameters[0].type_label, "Boolean");
    }
}

#[test]
fn invalid_return_contexts_do_not_publish_operand_facts() {
    for text in [
        "fun editing(value: Boolean) { return@other value. }",
        "fun editing(value: Boolean) = return value.",
    ] {
        let mut analysis = analyze(text);
        assert!(
            matches!(
                receiver(&mut analysis).value_type,
                V::Unavailable(U::BodyFailure)
            ),
            "{text}"
        );
    }
}
