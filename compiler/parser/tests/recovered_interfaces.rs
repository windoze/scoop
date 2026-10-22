use scoop_ast::{DeclarationFragment as Fragment, RecoveredNominal as Nominal};
use scoop_parser::parse_recovering;

#[test]
fn interface_defaults_keep_abstract_members_properties_and_nested_declarations() {
    let text = "interface Contract<T> {\nfun editing(value: T): T { value. }\nfun required(value: T): T\nval flag: Boolean\nclass Nested\ncompanion object { fun healthy() = true }\n}";
    let parsed = parse_recovering(text).unwrap();
    assert!(parsed.ast().is_none());
    assert_eq!(parsed.syntax().to_string(), text);
    let interface = parsed
        .declaration_data()
        .declarations
        .iter()
        .find_map(|item| {
            let Fragment::Nominal(Nominal::Interface(interface)) = &item.value else {
                return None;
            };
            Some(interface)
        })
        .unwrap();
    assert_eq!(interface.header.type_params[0].name.text, "T");
    assert_eq!(interface.methods.len(), 1);
    assert_eq!(
        interface.methods[0].header().body_form,
        scoop_ast::FunctionBodyForm::Omitted
    );
    assert_eq!(interface.properties.len(), 1);
    assert_eq!(interface.nested.len(), 1);
    assert!(interface.companion.is_some());
    assert_eq!(interface.recovery.methods.len(), 1);
    let method = &interface.recovery.methods[0];
    assert_eq!(method.header.signature.name.text, "editing");
    assert_eq!(method.body.as_ref().unwrap().selectors.len(), 1);
}

#[test]
fn interface_recovery_obeys_original_nested_owners_at_eof() {
    for text in [
        "class Outer { interface Inner<T> { fun editing(value: T) { value.",
        "struct Outer(val flag: Boolean) { interface Inner<T> { fun editing(value: T) { value.",
        "interface Outer { interface Inner<T> { fun editing(value: T) { value.",
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
        };
        let [Nominal::Interface(inner)] = nested.as_slice() else {
            panic!("original nested interface");
        };
        assert_eq!(inner.header.name.text, "Inner");
        assert_eq!(inner.recovery.methods.len(), 1);
        assert_eq!(inner.span.end as usize, text.len());
    }
}

#[test]
fn unknown_interface_headers_and_members_do_not_form_smaller_interfaces() {
    for text in [
        "interface Missing",
        "interface Broken<T: > { fun editing(value: T) { value. } }",
        "interface Broken { fun editing(value: ) { value. } }",
        "interface Broken { val flag: Boolean get() = true. }",
        "interface Broken { init {}\nfun editing() { true. } }",
        "interface Broken { constructor() {}\nfun editing() { true. } }",
        "interface Broken { release {}\nfun editing() { true. } }",
        "interface Broken { companion object {}\ncompanion object {} }",
    ] {
        let parsed = parse_recovering(text).unwrap();
        assert!(parsed.ast().is_none(), "{text}");
        assert!(
            !parsed
                .declaration_data()
                .declarations
                .iter()
                .any(|item| { matches!(item.value, Fragment::Nominal(_)) }),
            "{text}"
        );
        assert!(parsed.declaration_data().declarations.iter().any(|item| {
            matches!(&item.value, Fragment::Unavailable(header) if matches!(header.name.text.as_str(), "Broken" | "Missing"))
        }), "{text}");
    }
}
