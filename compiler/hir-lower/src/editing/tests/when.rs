use super::receivers::{known_type, receiver};
use super::*;

#[test]
fn when_bodies_use_the_original_subject_and_ordered_condition_paths() {
    for text in [
        "fun editing(value: Any) { when (value) { is String -> { value. } } }",
        "fun editing(value: Any) { when (value) { !is String -> {}\nelse -> value. } }",
        "fun editing(value: Any) { when { value is String -> value. } }",
        "fun editing(value: Any) { when { value !is String -> {}\nelse -> value. } }",
        "fun editing(value: Any) = when (value) { is String -> value.\nelse -> false }",
        "fun editing(value: Any) { val result = when (value) { is String -> value.\nelse -> false } }",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
    }
}

#[test]
fn subject_declarations_preserve_shadowing_and_irrefutable_binding_sources() {
    for text in [
        "fun editing(source: Any, value: Boolean) { when (val value = source) { is String -> value. } }",
        "fun editing() { when (val (flag, value) = (true, \"ready\")) { case (_, _) -> value. } }",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
        let definition = facts.local.unwrap().definition.span;
        assert_eq!(
            &text[definition.start as usize..definition.end as usize],
            "value"
        );
        assert!(definition.start as usize > text.find("when").unwrap());
    }
}

#[test]
fn case_bindings_use_tuple_struct_and_enum_declarations() {
    for text in [
        "fun editing(source: (Boolean, String)) { when (source) { case (flag, value) -> value. } }",
        "struct Record(val field: String)\nfun editing(source: Record) { when (source) { case Record { field: value } -> value. } }",
        "enum Item<T> { Empty, Full(T) }\nfun editing(source: Item<String>) { when (source) { case Full(value) -> value. } }",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
    }
}

#[test]
fn when_branches_isolate_body_failures_shadowing_and_mutable_facts() {
    for (prefix, first) in [
        ("val value = \"inner\"", Some("String")),
        ("val copy = missing", None),
    ] {
        let text = format!(
            "fun editing(value: Boolean) {{ when {{ value -> {{ {prefix}; value. }}\nelse -> {{ value. }} }}; value. }}"
        );
        let mut analysis = analyze(&text);
        let id = function(&analysis, "editing");
        let facts = analysis.receivers(id, || false).unwrap().to_vec();
        assert_eq!(facts.len(), 3);
        if let Some(expected) = first {
            assert_eq!(known_type(&mut analysis, &facts[0]), expected);
        } else {
            assert!(matches!(
                facts[0].value_type,
                V::Unavailable(U::BodyFailure)
            ));
        }
        assert_eq!(known_type(&mut analysis, &facts[1]), "Boolean");
        assert!(matches!(
            facts[2].value_type,
            V::Unavailable(U::IncompleteSyntax)
        ));
    }
    let mut analysis = analyze(
        "fun editing(value: Any) { var local: Any = value; when { local is String -> { local = true; local. }\nelse -> local. } }",
    );
    let id = function(&analysis, "editing");
    let facts = analysis.receivers(id, || false).unwrap().to_vec();
    assert_eq!(facts.len(), 2);
    assert_ne!(known_type(&mut analysis, &facts[0]), "String");
    assert_eq!(known_type(&mut analysis, &facts[1]), "Any");
}

#[test]
fn guarded_arms_keep_primary_bindings_and_guard_false_paths() {
    for text in [
        "fun editing(value: Any, flag: Boolean) { when (value) { is String if flag -> value. } }",
        "fun editing(source: (Boolean, String)) { when (source) { case (flag, value) if flag -> value. } }",
        "fun editing(value: Any) { when (value) { else if value !is String -> {}\nelse -> value. } }",
    ] {
        let mut analysis = analyze(text);
        let facts = receiver(&mut analysis);
        assert_eq!(known_type(&mut analysis, &facts), "String", "{text}");
    }
}

#[test]
fn failed_subjects_conditions_patterns_and_guards_do_not_publish_branch_bindings() {
    for text in [
        "fun editing(value: String) { when (missing) { else -> value. } }",
        "fun editing(value: String) { when { \"not Boolean\" -> value. } }",
        "fun editing(source: (Boolean, String)) { when (source) { case (value, value) -> value. } }",
        "fun editing(source: (Boolean, String)) { when (source) { case (flag, value) if \"not Boolean\" -> value. } }",
        "fun editing(value: String) { when { missing -> {}\nelse -> value. } }",
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

#[test]
fn nested_when_queries_keep_their_own_subjects_and_scopes() {
    let text = "fun editing(value: Any) { when (value) { is String -> { when (val flag = true) { true -> { value. } } } } }";
    let mut analysis = analyze(text);
    let facts = receiver(&mut analysis);
    assert_eq!(known_type(&mut analysis, &facts), "String");
}
