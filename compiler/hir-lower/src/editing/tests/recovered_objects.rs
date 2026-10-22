use super::*;
use crate::Owner;

#[test]
fn recovered_object_methods_keep_members_conformance_and_sibling_bodies() {
    let text = "interface Contract { fun healthy(): Boolean }\nobject Factory: Contract {\nval enabled: Boolean = true\npublic override fun healthy(): Boolean = enabled\nprivate fun hidden(): Boolean = true\nfun editing() { this. }\n}\nfun consumer(): Boolean = Factory.healthy()\nfun outside(value: Factory) { value. }";
    let mut analysis = analyze(text);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    let members = members::members(&mut analysis);
    for name in ["enabled", "hidden", "healthy", "editing"] {
        assert!(
            members.candidates.iter().any(|member| member.name == name),
            "{name}: {members:?}"
        );
    }
    let consumer = function(&analysis, "consumer");
    let V::Known(facts) = analysis.function_facts(consumer, || false).unwrap() else {
        panic!("complete object consumer")
    };
    assert_eq!(facts.callables.len(), 1);
    let hir::SemanticCallable::Current(healthy) = facts.callables[0].target else {
        panic!("source method")
    };
    assert!(matches!(
        analysis.function_facts(healthy, || false).unwrap(),
        V::Known(_)
    ));
    let outside = function(&analysis, "outside");
    let receivers = analysis.receivers(outside, || false).unwrap();
    let V::Known(members) = &receivers[0].members else {
        panic!("object receiver")
    };
    assert!(
        !members
            .candidates
            .iter()
            .any(|member| member.name == "hidden")
    );
}

#[test]
fn recovered_companions_keep_the_host_generic_binder_and_applications() {
    for host in [
        "class Host<T> {",
        "struct Host<T>(val field: T) {",
        "interface Host<T> {",
        "enum Host<T> { Value(T)\n",
    ] {
        let text = format!(
            "{host}\ncompanion object Factory {{\nfun editing(value: T): T {{ value. }}\nfun healthy(value: T): T = value\n}}\n}}\nfun consumer(): Boolean = Host<Boolean>.healthy(true)"
        );
        let mut analysis = analyze(&text);
        assert!(
            analysis.diagnostics().is_empty(),
            "{text}: {:?}",
            analysis.diagnostics()
        );
        let receiver = receivers::receiver(&mut analysis);
        assert_eq!(
            receivers::known_type(&mut analysis, &receiver),
            "T",
            "{text}"
        );
        let editing = function(&analysis, "editing");
        let Owner::Object(owner) = analysis.lowerer.function_owner[&editing] else {
            panic!("companion method owner")
        };
        assert!(analysis.lowerer.companion_host(owner).is_some());
        for name in ["healthy", "consumer"] {
            let function = function(&analysis, name);
            let facts = analysis.function_facts(function, || false).unwrap();
            assert!(matches!(facts, V::Known(_)), "{text}: {name}: {facts:?}");
        }
    }
}

#[test]
fn recovered_nested_singleton_keeps_its_own_method_type_parameters() {
    for tail in [" } } }", ""] {
        let text =
            format!("class Outer<T> {{ object Inner {{ fun <U> editing(value: U) {{ value.{tail}");
        let mut analysis = analyze(&text);
        assert!(
            analysis.diagnostics().is_empty(),
            "{:?}",
            analysis.diagnostics()
        );
        let receiver = receivers::receiver(&mut analysis);
        assert_eq!(receivers::known_type(&mut analysis, &receiver), "U");
        let editing = function(&analysis, "editing");
        let Owner::Object(owner) = analysis.lowerer.function_owner[&editing] else {
            panic!("object owner")
        };
        assert_eq!(analysis.lowerer.objects[owner].name, "Inner");
        assert!(analysis.lowerer.objects[owner].owner.is_some());
    }
}

#[test]
fn unresolved_recovered_object_methods_block_an_overload_choice() {
    let text = "object Subject {\nfun select(value: Missing): Boolean { value. }\nfun select(value: Boolean): Boolean = value\n}\nfun editing(value: Subject) { value. }\nfun consumer(): Boolean = Subject.select(true)";
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
