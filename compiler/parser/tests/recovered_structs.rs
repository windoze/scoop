use scoop_ast::{DeclarationFragment as Fragment, RecoveredNominal as Nominal};
use scoop_parser::parse_recovering;

#[test]
fn recovered_struct_retains_its_representation_and_complete_member_syntax() {
    let text = "struct Box<T>(val item: T, val flag: Boolean = true) {\nfun editing(value: T): T { value. }\nfun healthy(value: T): T = value\nval property: Boolean get() = flag\n}";
    let parsed = parse_recovering(text).unwrap();
    assert!(parsed.ast().is_none());
    assert_eq!(parsed.syntax().to_string(), text);
    let structure = parsed
        .declaration_data()
        .declarations
        .iter()
        .find_map(|item| {
            let Fragment::Nominal(Nominal::Struct(structure)) = &item.value else {
                return None;
            };
            Some(structure)
        })
        .unwrap();
    assert_eq!(structure.header.fields.len(), 2);
    assert_eq!(structure.header.fields[0].name.text, "item");
    assert!(matches!(
        structure.header.fields[1].syntax,
        scoop_ast::ParameterSyntax::Default { .. }
    ));
    assert_eq!(structure.members.len(), 2);
    assert_eq!(structure.recovery.methods.len(), 1);
    assert_eq!(
        structure.recovery.methods[0].header.signature.name.text,
        "editing"
    );
    assert_eq!(
        structure.recovery.methods[0]
            .body
            .as_ref()
            .unwrap()
            .selectors
            .len(),
        1
    );
}

#[test]
fn recovered_class_and_struct_nesting_uses_the_original_member_productions() {
    for text in [
        "class Outer { struct Inner<T>(val value: T) { fun editing() { value.",
        "struct Outer(val value: Boolean) { class Inner<T> { fun editing(value: T) { value.",
    ] {
        let parsed = parse_recovering(text).unwrap();
        let nominal = parsed
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
        let (outer, inner) = match nominal {
            Nominal::Class(outer) => (outer.header.name.text.as_str(), &outer.recovery.nested[0]),
            Nominal::Struct(outer) => (outer.header.name.text.as_str(), &outer.recovery.nested[0]),
        };
        assert_eq!(outer, "Outer");
        let (name, methods) = match inner {
            Nominal::Class(inner) => (&inner.header.name.text, &inner.recovery.methods),
            Nominal::Struct(inner) => (&inner.header.name.text, &inner.recovery.methods),
        };
        assert_eq!(name, "Inner");
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].body.as_ref().unwrap().selectors.len(), 1);
        assert_eq!(inner.span().end as usize, text.len());
    }
}

#[test]
fn unknown_representation_defaults_and_constructors_do_not_form_a_partial_struct() {
    for damaged in [
        "struct Broken(val value: ) { fun editing() { true. } }",
        "struct Broken(val value: Boolean = ) { fun editing() { true. } }",
        "struct Broken(val value: Boolean) { constructor(value: Boolean): this(value) { value. } }",
        "struct Broken(val value: Boolean) { val property: Boolean get() = value. }",
        "struct Broken(val value: Boolean) { fun editing(value: ) { value. } }",
    ] {
        let text = format!("{damaged}\nfun independent() = true");
        let parsed = parse_recovering(&text).unwrap();
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
