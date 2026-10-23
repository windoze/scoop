use super::imports::{analyze_files, at};
use super::*;
use hir::{EditingCallableIdentity as Identity, EditingReferenceTarget as Target};

mod failures;
mod scopes;

fn analyze_accessors(text: &str) -> DeclarationAnalysis {
    analyze_files(&[("src/accessors.scoop", text)])
}

fn region(analysis: &mut DeclarationAnalysis, text: &str, needle: &str) -> hir::EditingSourceBody {
    let location = at("src/accessors.scoop", text, needle, needle.len());
    analysis
        .source_body_at(&location.source, location.span, || false)
        .unwrap()
        .unwrap_or_else(|| panic!("missing body at {needle}: {:?}", analysis.diagnostics()))
}

fn facts(
    analysis: &mut DeclarationAnalysis,
    body: &hir::EditingSourceBody,
) -> hir::EditingBodyFacts {
    match analysis.source_body_facts(body.id, || false).unwrap() {
        V::Known(facts) => facts.clone(),
        V::Unavailable(reason) => {
            panic!(
                "accessor facts unavailable: {reason:?}; {:?}",
                analysis.diagnostics()
            )
        }
    }
}

fn local_type(
    analysis: &mut DeclarationAnalysis,
    body: &hir::EditingSourceBody,
    local: &hir::EditingLocalOccurrence,
) -> String {
    let V::Known(description) = analysis
        .describe_body_type(body.id, local.context, local.value_type)
        .unwrap()
    else {
        panic!("checked local preserves its actual type context")
    };
    description.name
}

#[test]
fn explicit_bodies_keep_backing_fields_calls_and_source_accessor_identities() {
    let text = "var state: Boolean = true\nget() { val saved = field; return pass(saved) }\nset(next) { field = pass(next) }\nfun pass(value: Boolean): Boolean = value";
    let mut analysis = analyze_accessors(text);
    let getter = region(&mut analysis, text, "pass(saved)");
    let setter = region(&mut analysis, text, "pass(next)");
    assert!(matches!(
        getter.callable(),
        Some(hir::SemanticCallable::CurrentGetter(_))
    ));
    assert!(matches!(
        setter.callable(),
        Some(hir::SemanticCallable::CurrentSetter(_))
    ));
    assert_ne!(getter.id, setter.id);
    assert_eq!(analysis.functions().len(), 1);
    let getter_facts = facts(&mut analysis, &getter);
    let setter_facts = facts(&mut analysis, &setter);
    let pass =
        hir::EditingCallee::Callable(hir::SemanticCallable::Current(function(&analysis, "pass")));
    for body in [&getter_facts, &setter_facts] {
        assert_eq!(body.callables.len(), 1);
        assert_eq!(body.callables[0].target, pass);
    }
    let saved = getter_facts
        .locals
        .iter()
        .find(|local| local.reference.name == "saved" && local.role == hir::EditingLocalRole::Read)
        .unwrap();
    assert_eq!(local_type(&mut analysis, &getter, saved), "Boolean");
    assert_eq!(
        saved.reference.definition,
        at("src/accessors.scoop", text, "saved =", 5)
    );
    let next = setter_facts
        .locals
        .iter()
        .find(|local| local.reference.name == "next" && local.role == hir::EditingLocalRole::Read)
        .unwrap();
    assert_eq!(local_type(&mut analysis, &setter, next), "Boolean");
    assert_eq!(
        next.reference.definition,
        at("src/accessors.scoop", text, "next)", 4)
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let parameter = index.targets_at(&next.reference.definition);
    assert!(matches!(
        parameter,
        [Target::Parameter {
            callable: Identity::Declaration(hir::DefaultCallableDeclarationV1::PropertyAccessor(_)),
            index: 0
        }]
    ));
    assert_eq!(index.references_to(parameter[0]).count(), 2);
}

#[test]
fn querying_a_setter_first_resolves_its_real_inferred_property_type() {
    for text in [
        "var state = true\nget() = field\nset(next) { field = next }",
        "var state get() = true\nset(next) { val saved = next }",
    ] {
        let mut analysis = analyze_accessors(text);
        let setter = region(&mut analysis, text, "set(next)");
        let body = facts(&mut analysis, &setter);
        let next = body
            .locals
            .iter()
            .find(|local| {
                local.reference.name == "next" && local.role == hir::EditingLocalRole::Read
            })
            .unwrap();
        assert_eq!(
            local_type(&mut analysis, &setter, next),
            "Boolean",
            "{text}"
        );
        let V::Known(signature) = analysis.signature(setter.id.function().unwrap()) else {
            panic!("inferred setter has a complete signature")
        };
        assert_eq!(signature.parameters.len(), 1);
        let getter = region(&mut analysis, text, "get()");
        assert!(!facts(&mut analysis, &getter).expressions.is_empty());
        assert!(
            analysis.diagnostics().is_empty(),
            "{text}: {:?}",
            analysis.diagnostics()
        );
    }
}

#[test]
fn implicit_setter_parameters_keep_accessor_targets_without_exposing_generated_bodies() {
    let text = "class Box(var stored: Boolean)\nvar state: Boolean = true\nget() = field\nset() { field = value }\nval computed: Boolean get() = state";
    let mut analysis = analyze_accessors(text);
    assert!(analysis.functions().is_empty());
    assert_eq!(analysis.source_bodies().len(), 3);
    let setter = region(&mut analysis, text, "field = value");
    let body = facts(&mut analysis, &setter);
    let value = body
        .locals
        .iter()
        .find(|local| local.reference.name == "value" && local.role == hir::EditingLocalRole::Read)
        .unwrap();
    assert_eq!(local_type(&mut analysis, &setter, value), "Boolean");
    let usage = at("src/accessors.scoop", text, "value }", 5);
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    assert!(matches!(
        index.targets_at(&usage),
        [Target::Parameter {
            callable: Identity::Declaration(hir::DefaultCallableDeclarationV1::PropertyAccessor(_)),
            index: 0
        }]
    ));
}

#[test]
fn property_references_include_independent_getter_and_setter_source_uses() {
    let text = "var stored: Boolean = true\nvar projected: Boolean get() = stored\nset(next) { stored = next }\nfun editing(): Boolean = projected";
    let mut analysis = analyze_accessors(text);
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let origin = at("src/accessors.scoop", text, "stored:", 6);
    let target = index.targets_at(&origin)[0];
    let references = index.references_to(target).collect::<Vec<_>>();
    assert_eq!(references.len(), 3);
    assert_eq!(
        references
            .iter()
            .filter(|reference| reference.is_declaration)
            .count(),
        1
    );
}
