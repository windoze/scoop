use scoop_ast::{DeclarationFragment as Fragment, RecoveredNominal as Nominal};
use scoop_parser::parse_recovering;

fn recovered(text: &str) -> Nominal {
    let parsed = parse_recovering(text).unwrap();
    assert!(parsed.ast().is_none(), "{text}");
    assert_eq!(parsed.syntax().to_string(), text);
    parsed
        .declaration_data()
        .declarations
        .iter()
        .find_map(|item| {
            let Fragment::Nominal(nominal) = &item.value else {
                return None;
            };
            Some(nominal.clone())
        })
        .unwrap_or_else(|| panic!("missing recovered nominal: {text}"))
}

#[test]
fn recovered_objects_keep_stored_members_and_nested_declaration_owners() {
    let text = "object Factory: Contract {\nval enabled: Boolean = true\nfun editing() { this. }\nfun healthy(): Boolean = enabled\nobject Nested { fun inner(value: Boolean) { value. } }\n}";
    let Nominal::Object(object) = recovered(text) else {
        panic!("object source")
    };
    assert_eq!(object.header.name.text, "Factory");
    assert_eq!(object.header.supertypes.len(), 1);
    assert_eq!(object.members.len(), 2);
    assert_eq!(object.recovery.methods.len(), 1);
    let [Nominal::Object(nested)] = object.recovery.nested.as_slice() else {
        panic!("nested object")
    };
    assert_eq!(nested.header.name.text, "Nested");
    assert_eq!(
        nested.recovery.methods[0].header.signature.name.text,
        "inner"
    );
}

#[test]
fn recovered_companions_remain_inside_every_supported_host_at_eof() {
    for host in [
        "class Host<T> {",
        "struct Host<T> {",
        "interface Host<T> {",
        "enum Host<T> { Value(T)\n",
        "object Host {",
    ] {
        for tail in [" } } }", ""] {
            let text = format!(
                "{host}\ncompanion object Factory {{ fun editing(value: Boolean) {{ value.{tail}"
            );
            let mut nominal = recovered(&text);
            let companion = nominal.recovery_mut().companion.as_ref().unwrap();
            assert!(
                matches!(&companion.header.name, scoop_ast::CompanionNameSyntax::Named(name) if name.text == "Factory")
            );
            assert_eq!(companion.recovery.methods.len(), 1);
            assert_eq!(
                companion.span.end as usize,
                if tail.is_empty() {
                    text.len()
                } else {
                    text.len() - 2
                }
            );
        }
    }
}

#[test]
fn unknown_singleton_headers_initializers_and_constructors_are_not_omitted() {
    for text in [
        "object Broken<T> { fun editing() { true. } }",
        "object Broken { constructor() {} }",
        "object Broken { constructor() {}",
        "object Broken { val flag: Boolean =\nfun editing() { true. } }",
        "object Broken { init { fun local(value: Boolean) { value. } } }",
        "object Broken { fun editing(value: ) { true. } }",
        "class Broken { companion object { constructor() {} } }",
        "class Broken { companion object { constructor() {}",
        "class Broken { companion object {}\ncompanion object { fun editing() { true.",
        "enum Broken { Empty\ncompanion object {}\ncompanion object { fun editing() { true.",
    ] {
        let parsed = parse_recovering(text).unwrap();
        assert!(parsed.ast().is_none(), "{text}");
        assert!(
            !parsed
                .declaration_data()
                .declarations
                .iter()
                .any(|item| matches!(item.value, Fragment::Nominal(_))),
            "{text}"
        );
    }
}

#[test]
fn nested_types_inside_a_recovered_companion_keep_their_member_body() {
    let mut nominal = recovered(
        "class Host<T> { companion object { class Nested<U> { fun editing(value: U) { value.",
    );
    let companion = nominal.recovery_mut().companion.as_ref().unwrap();
    assert!(matches!(
        companion.header.name,
        scoop_ast::CompanionNameSyntax::Default { .. }
    ));
    let [Nominal::Class(nested)] = companion.recovery.nested.as_slice() else {
        panic!("nested class in the companion")
    };
    assert_eq!(nested.header.name.text, "Nested");
    assert_eq!(nested.recovery.methods.len(), 1);
}
