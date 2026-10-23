use scoop_ast::{DeclarationFragment as Fragment, RecoveredNominal as Nominal};
use scoop_parser::parse_recovering;

#[test]
fn recovered_enums_keep_every_variant_form_and_complete_member() {
    let text = "enum Choice<T> {\nEmpty, Positional(Boolean), Named { flag: Boolean }, Full(val value: T, val enabled: Boolean = true)\nfun editing(value: T): T { value. }\nfun healthy(value: T): T = value\nval flag: Boolean get() = true\n}";
    let parsed = parse_recovering(text).unwrap();
    assert!(parsed.ast().is_none());
    assert_eq!(parsed.syntax().to_string(), text);
    let enumeration = parsed
        .declaration_data()
        .declarations
        .iter()
        .find_map(|item| {
            let Fragment::Nominal(Nominal::Enum(enumeration)) = &item.value else {
                return None;
            };
            Some(enumeration)
        })
        .unwrap();
    assert_eq!(
        enumeration
            .variants
            .iter()
            .map(|variant| variant.name.text.as_str())
            .collect::<Vec<_>>(),
        ["Empty", "Positional", "Named", "Full"]
    );
    let scoop_ast::VariantDeclKind::Constructor(fields) = &enumeration.variants[3].kind else {
        panic!("original constructor-style variant");
    };
    assert!(matches!(
        fields[1].syntax,
        scoop_ast::ParameterSyntax::Default { .. }
    ));
    assert_eq!(enumeration.methods.len(), 1);
    assert_eq!(enumeration.properties.len(), 1);
    assert_eq!(enumeration.recovery.methods.len(), 1);
    assert_eq!(
        enumeration.recovery.methods[0]
            .body
            .as_ref()
            .unwrap()
            .selectors
            .len(),
        1
    );
}

#[test]
fn recovered_enum_members_and_nested_types_keep_the_original_body_owner() {
    for text in [
        "class Outer { enum Inner<T> { Full(T)\nfun editing(value: T) { value.",
        "interface Outer { enum Inner<T> { Full(T)\nfun editing(value: T) { value.",
        "enum Outer { Empty\nenum Inner<T> { Full(T)\nfun editing(value: T) { value.",
    ] {
        let parsed = parse_recovering(text).unwrap();
        let outer = parsed
            .declaration_data()
            .declarations
            .iter()
            .find_map(|item| {
                let Fragment::Nominal(nominal) = &item.value else {
                    return None;
                };
                Some(nominal)
            })
            .unwrap();
        let nested = match outer {
            Nominal::Class(outer) => &outer.recovery.nested,
            Nominal::Struct(outer) => &outer.recovery.nested,
            Nominal::Interface(outer) => &outer.recovery.nested,
            Nominal::Enum(outer) => &outer.recovery.nested,
            Nominal::Object(outer) => &outer.recovery.nested,
        };
        let [Nominal::Enum(inner)] = nested.as_slice() else {
            panic!("original nested enum")
        };
        assert_eq!(inner.header.name.text, "Inner");
        assert_eq!(inner.variants.len(), 1);
        assert_eq!(inner.recovery.methods.len(), 1);
        assert_eq!(inner.span.end as usize, text.len());
    }
}

#[test]
fn unknown_enum_variants_defaults_and_members_are_not_dropped() {
    for text in [
        "enum Broken",
        "enum Broken { Full(val value: )\nfun editing() { true. } }",
        "enum Broken { Full(val value: Boolean = )\nfun editing() { true. } }",
        "enum Broken : Int { First =\nfun editing() { true. } }",
        "enum Broken { Empty\nfun editing(value: ) { true. } }",
        "enum Broken { Empty\nval flag: Boolean get() = true. }",
        "enum Broken { Empty\ncompanion object {}\ncompanion object {} }",
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
        assert!(parsed.declaration_data().declarations.iter().any(|item| matches!(&item.value, Fragment::Unavailable(header) if header.name.text == "Broken")), "{text}");
    }
}

#[test]
fn lost_nominal_item_boundaries_do_not_publish_a_smaller_declaration() {
    for text in [
        "enum Broken { First Second\nfun editing() { true. } }",
        "class Broken { fun first() {} fun skipped() {}\nfun editing() { true. } }",
        "struct Broken(val flag: Boolean) { fun first() {} fun skipped() {}\nfun editing() { true. } }",
        "interface Broken { fun first() {} fun skipped() {}\nfun editing() { true. } }",
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
