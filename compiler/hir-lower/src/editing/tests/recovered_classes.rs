use super::*;
use crate::Owner;

#[test]
fn recovered_class_methods_keep_receiver_scope_and_complete_sibling_bodies() {
    let text = "open class Parent { protected fun inherited(): Boolean = true }\nclass Child(val value: Boolean): Parent() {\nprivate fun hidden(): Boolean = true\nfun editing() { this. }\nfun healthy(): Boolean = hidden()\n}\nfun outside(value: Child) { value. }";
    let mut analysis = analyze(text);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    let members = members::members(&mut analysis);
    for name in ["value", "hidden", "inherited", "healthy", "editing"] {
        assert!(
            members
                .candidates
                .iter()
                .any(|candidate| candidate.name == name),
            "{name}: {members:?}"
        );
    }
    let healthy = function(&analysis, "healthy");
    let V::Known(facts) = analysis.function_facts(healthy, || false).unwrap() else {
        panic!("the complete sibling body is independently checkable");
    };
    assert_eq!(facts.callables.len(), 1);
    let editing = function(&analysis, "editing");
    assert!(matches!(
        analysis.function_facts(editing, || false).unwrap(),
        V::Unavailable(U::IncompleteSyntax)
    ));
    let outside = function(&analysis, "outside");
    let receivers = analysis.receivers(outside, || false).unwrap();
    let V::Known(members) = &receivers[0].members else {
        panic!("known class receiver")
    };
    assert!(
        !members
            .candidates
            .iter()
            .any(|member| matches!(member.name.as_str(), "hidden" | "inherited"))
    );
}

#[test]
fn recovered_generic_override_and_its_application_share_the_original_signatures() {
    let text = "interface Contract<T> { fun identity(value: T): T }\nclass Box<T>(val field: T): Contract<T> {\npublic override fun identity(value: T): T { value. }\nfun sibling(value: T): T = value\n}\nfun consumer(value: Box<Boolean>): Boolean = value.identity(true)";
    let mut analysis = analyze(text);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    let method = analysis
        .functions()
        .iter()
        .find(|function| {
            function.name == "identity"
                && function.name_origin.span.start as usize == text.rfind("identity(value").unwrap()
        })
        .unwrap()
        .function;
    let V::Known(signature) = analysis.signature(method) else {
        panic!("generic method signature")
    };
    let ty = signature.parameters[0].value_type;
    let V::Known(description) = analysis.describe_type(method, ty).unwrap() else {
        panic!("original owner binder")
    };
    assert_eq!(description.name, "T");
    let receivers = analysis.receivers(method, || false).unwrap();
    assert!(matches!(receivers[0].value_type, V::Known(receiver) if receiver == ty));
    let consumer = function(&analysis, "consumer");
    let V::Known(facts) = analysis.function_facts(consumer, || false).unwrap() else {
        panic!("known method application")
    };
    assert_eq!(facts.callables.len(), 1);
    assert_eq!(
        facts.callables[0].target,
        hir::SemanticCallable::Current(method)
    );
}

#[test]
fn recovered_inner_class_keeps_its_parent_type_parameters() {
    for tail in [" } } }", ""] {
        let text = format!(
            "class Outer<T>(val field: T) {{ inner class Inner {{ fun editing(value: T) {{ value.{tail}"
        );
        let mut analysis = analyze(&text);
        assert!(
            analysis.diagnostics().is_empty(),
            "{:?}",
            analysis.diagnostics()
        );
        let receiver = receivers::receiver(&mut analysis);
        assert_eq!(receivers::known_type(&mut analysis, &receiver), "T");
        let editing = function(&analysis, "editing");
        let Owner::Class(inner) = analysis.lowerer.function_owner[&editing] else {
            panic!("class method owner")
        };
        assert_eq!(analysis.lowerer.classes[inner].name, "Inner");
        assert!(analysis.lowerer.classes[inner].owner.is_some());
    }
}

#[test]
fn unresolved_recovered_method_signatures_still_block_an_overload_choice() {
    let text = "class Subject {\nfun select(value: Missing): Boolean { value. }\nfun select(value: Boolean): Boolean = value\n}\nfun editing(value: Subject) { value. }\nfun consumer(value: Subject): Boolean = value.select(true)";
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
