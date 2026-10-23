use super::*;
use crate::Owner;

#[test]
fn recovered_struct_representation_and_interface_methods_remain_usable() {
    let text = "interface Contract<T> { fun echo(value: T): T }\nstruct Box<T>(val stored: T, val flag: Boolean = true): Contract<T> {\npublic override fun echo(value: T): T { value. }\nfun editing() { this. }\nfun healthy(value: T): T = value\n}\nfun consumer(value: Box<Boolean>): Boolean = value.healthy(value.stored)";
    let mut analysis = analyze(text);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    let members = members::members(&mut analysis);
    for name in ["stored", "flag", "echo", "healthy", "editing"] {
        assert!(
            members.candidates.iter().any(|member| member.name == name),
            "{name}: {members:?}"
        );
    }
    let consumer = function(&analysis, "consumer");
    let V::Known(facts) = analysis.function_facts(consumer, || false).unwrap() else {
        panic!("known struct member application")
    };
    assert_eq!(facts.callables.len(), 1);
    assert_eq!(facts.properties.len(), 1);
    let property = facts.properties[0].clone();
    let V::Known(description) = analysis
        .describe_body_type(consumer, property.context, property.value_type)
        .unwrap()
    else {
        panic!("applied field type")
    };
    assert_eq!(description.name, "Boolean");
}

#[test]
fn mixed_recovered_nominals_keep_typed_owners_and_generic_contexts() {
    for (text, structure) in [
        (
            "class Outer { struct Inner<T>(val stored: T) { fun editing(value: T) { value. } } }",
            true,
        ),
        (
            "struct Outer(val flag: Boolean) { class Inner<T> { fun editing(value: T) { value. } } }",
            false,
        ),
    ] {
        let mut analysis = analyze(text);
        assert!(
            analysis.diagnostics().is_empty(),
            "{:?}",
            analysis.diagnostics()
        );
        let receiver = receivers::receiver(&mut analysis);
        assert_eq!(receivers::known_type(&mut analysis, &receiver), "T");
        let editing = function(&analysis, "editing");
        match analysis.lowerer.function_owner[&editing] {
            Owner::Struct(inner) if structure => {
                assert_eq!(analysis.lowerer.structs[inner].name, "Inner");
                assert!(analysis.lowerer.structs[inner].owner.is_some());
            }
            Owner::Class(inner) if !structure => {
                assert_eq!(analysis.lowerer.classes[inner].name, "Inner");
                assert!(analysis.lowerer.classes[inner].owner.is_some());
            }
            owner => panic!("unexpected nested owner: {owner:?}"),
        }
    }
}

#[test]
fn unresolved_recovered_struct_method_blocks_overload_selection() {
    let text = "struct Subject(val flag: Boolean) {\nfun select(value: Missing): Boolean { value. }\nfun select(value: Boolean): Boolean = value\n}\nfun editing(value: Subject) { value. }\nfun consumer(value: Subject): Boolean = value.select(true)";
    let mut analysis = analyze(text);
    let members = members::members(&mut analysis);
    assert!(
        members
            .unavailable
            .iter()
            .any(|member| member.name == "select"),
        "{members:?}"
    );
    let consumer = function(&analysis, "consumer");
    assert!(matches!(
        analysis.function_facts(consumer, || false).unwrap(),
        V::Unavailable(U::BodyFailure)
    ));
}
