use super::*;
use crate::Owner;

#[test]
fn recovered_interface_defaults_preserve_slot_order_private_scope_and_sibling_bodies() {
    let text = "interface Contract<T> {\nfun editing(value: T): T { value. }\nfun required(): Boolean\nprivate fun secret(): Boolean = true\nfun members() { this. }\nfun healthy(): Boolean = secret()\n}\nfun outside(value: Contract<Boolean>) { value. }";
    let mut analysis = analyze(text);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    let editing = function(&analysis, "editing");
    let Owner::Interface(owner) = analysis.lowerer.function_owner[&editing] else {
        panic!("interface owner");
    };
    let methods = analysis.lowerer.interfaces[owner]
        .methods
        .iter()
        .map(|member| {
            let member = &analysis.lowerer.interface_method_entities[*member];
            (
                analysis.lowerer.source_function_declarations[&member.function]
                    .name
                    .as_str(),
                member.implementation,
            )
        })
        .collect::<Vec<_>>();
    use hir::InterfaceMemberImplementation::{AbstractSlot, Body};
    assert_eq!(
        methods,
        [
            ("editing", Body),
            ("required", AbstractSlot),
            ("members", Body),
            ("healthy", Body)
        ]
    );
    let inside = function(&analysis, "members");
    let receivers = analysis.receivers(inside, || false).unwrap();
    let V::Known(members) = &receivers[0].members else {
        panic!("known interface members")
    };
    assert!(
        members
            .candidates
            .iter()
            .any(|member| member.name == "secret")
    );
    let outside = function(&analysis, "outside");
    let receivers = analysis.receivers(outside, || false).unwrap();
    let V::Known(members) = &receivers[0].members else {
        panic!("known public interface members")
    };
    assert!(
        !members
            .candidates
            .iter()
            .any(|member| member.name == "secret")
    );
    let healthy = function(&analysis, "healthy");
    let secret = function(&analysis, "secret");
    let V::Known(facts) = analysis.function_facts(healthy, || false).unwrap() else {
        panic!("independent complete default body");
    };
    assert_eq!(
        facts.callables[0].target,
        hir::SemanticCallable::Current(secret)
    );
    let receiver = analysis.receivers(editing, || false).unwrap()[0].clone();
    assert_eq!(receivers::known_type(&mut analysis, &receiver), "T");
}

#[test]
fn recovered_default_implementation_keeps_the_original_provider_and_conformance() {
    let text = "interface Root<T> { fun identity(rootValue: T): T }\ninterface Defaults<T> : Root<T> { override fun identity(defaultValue: T): T { defaultValue. } }\nclass UsesDefault : Defaults<Boolean> {}\nfun consumer(value: UsesDefault): Boolean = value.identity(true)";
    let mut analysis = imports::analyze_files(&[("src/editing.scoop", text)]);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    let mut targets = Vec::new();
    for needle in ["identity(rootValue", "identity(defaultValue"] {
        let origin = imports::at("src/editing.scoop", text, needle, 8);
        let V::Known(values) = analysis
            .member_implementation_targets(&origin.source, origin.span, || false)
            .unwrap()
        else {
            panic!("original method target");
        };
        assert_eq!(values.len(), 1);
        targets.push(values[0]);
    }
    let V::Known(index) = analysis.member_implementation_index(|| false).unwrap() else {
        panic!("known implementation relation")
    };
    assert!(
        index
            .relations
            .contains(&hir::EditingMemberImplementationRelation {
                requirement: targets[0],
                implementation: targets[1],
            })
    );
    assert!(
        index
            .implementations
            .iter()
            .any(|item| item.declaration == targets[1])
    );
    assert!(
        !index
            .implementations
            .iter()
            .any(|item| item.declaration == targets[0])
    );
    let consumer = function(&analysis, "consumer");
    assert!(matches!(
        analysis.function_facts(consumer, || false).unwrap(),
        V::Known(_)
    ));
}

#[test]
fn recovered_interface_nesting_retains_its_own_type_parameters() {
    for text in [
        "class Outer<T> { interface Inner<U> { fun editing(value: U) { value. } } }",
        "struct Outer<T>(val field: T) { interface Inner<U> { fun editing(value: U) { value. } } }",
        "interface Outer<T> { class Inner<U> { fun editing(value: U) { value. } } }",
        "interface Outer<T> { struct Inner<U>(val field: U) { fun editing(value: U) { value. } } }",
    ] {
        let mut analysis = analyze(text);
        assert!(
            analysis.diagnostics().is_empty(),
            "{:?}",
            analysis.diagnostics()
        );
        let receiver = receivers::receiver(&mut analysis);
        assert_eq!(receivers::known_type(&mut analysis, &receiver), "U");
    }
}

#[test]
fn unresolved_interface_method_signatures_block_overload_selection() {
    let text = "interface Subject {\nfun select(value: Missing): Boolean { value. }\nfun select(value: Boolean): Boolean = value\n}\nfun editing(value: Subject) { value. }\nfun consumer(value: Subject): Boolean = value.select(true)";
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
