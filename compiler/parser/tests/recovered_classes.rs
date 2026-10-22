use scoop_ast::{ClassMember, DeclarationFragment as Fragment};
use scoop_parser::parse_recovering;

#[test]
fn class_recovery_keeps_complete_headers_members_and_method_bodies_separate() {
    let text = "public class Box<T>(val value: T) {\nfun broken(item: T): T { item. }\nfun intact(item: T): T = item\nval flag: Boolean = true\n}";
    let parsed = parse_recovering(text).unwrap();
    assert!(parsed.ast().is_none());
    assert_eq!(parsed.syntax().to_string(), text);
    let recovered = parsed
        .declaration_data()
        .declarations
        .iter()
        .find_map(|item| {
            let Fragment::Class(class) = &item.value else {
                return None;
            };
            Some(class)
        })
        .unwrap();
    assert_eq!(recovered.header.name.text, "Box");
    assert_eq!(recovered.header.type_params[0].name.text, "T");
    assert_eq!(recovered.header.constructor[0].name.text, "value");
    assert_eq!(recovered.methods.len(), 1);
    assert_eq!(recovered.methods[0].header.signature.name.text, "broken");
    assert_eq!(
        recovered.methods[0].body.as_ref().unwrap().selectors.len(),
        1
    );
    assert_eq!(recovered.members.len(), 2);
    assert!(
        matches!(&recovered.members[0], ClassMember::Function(function) if function.name.text == "intact")
    );
    assert!(
        matches!(&recovered.members[1], ClassMember::StoredProperty(property) if property.name.text == "flag")
    );
}

#[test]
fn nested_class_recovery_preserves_the_actual_container_even_at_eof() {
    let text = "class Outer<T> { inner class Inner { fun editing(value: T) { value.";
    let parsed = parse_recovering(text).unwrap();
    let classes = parsed
        .declaration_data()
        .declarations
        .iter()
        .filter_map(|item| match &item.value {
            Fragment::Class(class) => Some(class),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(classes.len(), 1, "{:?}", parsed.diagnostics());
    assert_eq!(classes[0].header.name.text, "Outer");
    let inner = &classes[0].classes[0];
    assert_eq!(inner.header.name.text, "Inner");
    assert!(inner.header.inner_span.is_some());
    assert_eq!(inner.methods[0].header.signature.name.text, "editing");
    assert_eq!(inner.methods[0].body.as_ref().unwrap().selectors.len(), 1);
    assert_eq!(inner.span.end as usize, text.len());
}

#[test]
fn incomplete_headers_or_non_method_members_do_not_form_smaller_types() {
    for damaged in [
        "class Broken<T: > { fun editing() { true. } }",
        "class Broken { fun editing(value: ) { value. } }",
        "class Broken { val value: = true\nfun editing() { true. } }",
        "class Broken { init { fun local() { true. } } }",
        "class Broken { val value = { fun local() { true. } } }",
        "class Broken { companion object {}\ncompanion object {} }",
    ] {
        let text = format!("{damaged}\nfun independent() = true");
        let parsed = parse_recovering(&text).unwrap();
        assert!(parsed.ast().is_none(), "{text}");
        assert!(
            !parsed
                .declaration_data()
                .declarations
                .iter()
                .any(|item| matches!(&item.value, Fragment::Class(_))),
            "{text}"
        );
        assert!(parsed.declaration_data().declarations.iter().any(|item| matches!(&item.value, Fragment::Unavailable(header) if header.name.text == "Broken")), "{text}");
        assert!(parsed.declaration_data().declarations.iter().any(|item| matches!(&item.value, Fragment::Complete(declaration) if matches!(declaration.as_ref(), scoop_ast::Decl::Function(function) if function.name.text == "independent"))), "{text}");
    }
}

#[test]
fn class_recovery_does_not_duplicate_complete_nested_function_declarations() {
    let text = "class Holder { init { fun local() = true\nlocal() }\nfun intact() { fun nested() = false\nnested() }\nfun editing() { true. } }";
    let parsed = parse_recovering(text).unwrap();
    let class = parsed
        .declaration_data()
        .declarations
        .iter()
        .find_map(|item| {
            let Fragment::Class(class) = &item.value else {
                return None;
            };
            Some(class)
        })
        .unwrap();
    assert_eq!(class.methods.len(), 1);
    assert_eq!(class.methods[0].header.signature.name.text, "editing");
    assert_eq!(class.members.len(), 2);
    assert!(matches!(class.members[0], ClassMember::InitBlock(_)));
    assert!(
        matches!(&class.members[1], ClassMember::Function(function) if function.name.text == "intact")
    );
}
