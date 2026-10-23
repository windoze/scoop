use super::imports::{analyze_files, at};
use super::*;
use hir::{
    EditingConstructionBody as Construction, EditingReferenceTarget as Target,
    EditingSourceBodyId as Body,
};

mod contexts;
mod failures;
mod imported;

fn analyze_construction(text: &str) -> DeclarationAnalysis {
    analyze_files(&[("src/construction.scoop", text)])
}

fn location(text: &str, needle: &str, length: usize) -> hir::SemanticSourceLocation {
    at("src/construction.scoop", text, needle, length)
}

fn region(analysis: &mut DeclarationAnalysis, text: &str, needle: &str) -> hir::EditingSourceBody {
    let origin = location(text, needle, 0);
    let body = analysis
        .source_body_at(&origin.source, origin.span, || false)
        .unwrap()
        .unwrap_or_else(|| {
            panic!(
                "missing construction at {needle}: {:?}",
                analysis.diagnostics()
            )
        });
    assert!(matches!(body.id, Body::Construction(_)), "{:?}", body.id);
    body
}

fn facts(
    analysis: &mut DeclarationAnalysis,
    body: &hir::EditingSourceBody,
) -> hir::EditingBodyFacts {
    match analysis.source_body_facts(body.id, || false).unwrap() {
        V::Known(facts) => facts.clone(),
        V::Unavailable(reason) => panic!(
            "construction unavailable: {reason:?}; {:?}",
            analysis.diagnostics()
        ),
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
        panic!("a constructor retains its actual binder context")
    };
    description.name
}

#[test]
fn constructor_regions_share_real_parameter_bindings_and_call_arguments() {
    let text = "fun <T> pick(value: T): T = value\nclass Box(seed: Boolean) {\nval ready: Boolean = pick(value = seed)\ninit { val local = pick(value = ready) }\nconstructor(copy: String): this(seed = true) { val local = pick(value = copy) }\n}";
    let mut analysis = analyze_construction(text);
    let property = region(&mut analysis, text, "pick(value = seed)");
    assert!(matches!(
        property.id,
        Body::Construction(Construction::Property(_))
    ));
    let observed = facts(&mut analysis, &property);
    let seed = observed
        .locals
        .iter()
        .find(|local| local.reference.name == "seed")
        .unwrap();
    assert_eq!(
        seed.reference.definition,
        location(text, "seed: Boolean", 4)
    );
    assert_eq!(local_type(&mut analysis, &property, seed), "Boolean");
    assert_eq!(
        analysis.body_parameter(&property, &seed.reference.definition),
        Some((property.callee().unwrap(), 0))
    );

    let init = region(&mut analysis, text, "pick(value = ready)");
    let observed = facts(&mut analysis, &init);
    assert!(matches!(
        init.id,
        Body::Construction(Construction::InitBlock { .. })
    ));
    assert_eq!(observed.properties.len(), 1);
    assert_eq!(
        observed.properties[0].definition,
        Some(location(text, "ready: Boolean", 5))
    );

    let secondary = region(&mut analysis, text, "pick(value = copy)");
    let observed = facts(&mut analysis, &secondary);
    let copy = observed
        .locals
        .iter()
        .find(|local| local.reference.name == "copy")
        .unwrap();
    assert_eq!(copy.reference.definition, location(text, "copy: String", 4));
    assert_eq!(local_type(&mut analysis, &secondary, copy), "String");

    let delegation = region(&mut analysis, text, "this(seed = true)");
    let V::Known(calls) = analysis.calls(delegation.id, || false).unwrap() else {
        panic!("actual delegation call")
    };
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].callee, property.callee());
    assert_eq!(
        calls[0].callee_span,
        Some(location(text, "this(seed", 4).span)
    );
    let V::Known(Some(argument)) = analysis
        .named_argument(delegation.id, location(text, "seed = true", 4).span, || {
            false
        })
        .unwrap()
    else {
        panic!("delegation keeps named arguments")
    };
    assert_eq!(
        argument.definition,
        Some(location(text, "seed: Boolean", 4))
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let target = index.targets_at(&seed.reference.definition)[0];
    assert!(matches!(target, Target::Parameter { index: 0, .. }));
    assert_eq!(index.references_to(target).count(), 3);
    let pick = index.targets_at(&location(text, "pick(value: T)", 4))[0];
    assert_eq!(index.references_to(pick).count(), 4);
}

#[test]
fn generic_base_delegation_and_inherited_fields_keep_the_selected_owner() {
    let text = "fun <T> pick(value: T): T = value\nopen class Base<T>(val base: T)\nclass Derived<T>(seed: T): Base<T>(base = pick(seed)) { val copy: T = base }";
    let mut analysis = analyze_construction(text);
    let delegation = region(&mut analysis, text, "Base<T>(base =");
    let observed = facts(&mut analysis, &delegation);
    let seed = observed
        .locals
        .iter()
        .find(|local| local.reference.name == "seed")
        .unwrap();
    assert_eq!(local_type(&mut analysis, &delegation, seed), "T");
    let V::Known(Some(argument)) = analysis
        .named_argument(delegation.id, location(text, "base = pick", 4).span, || {
            false
        })
        .unwrap()
    else {
        panic!("base constructor parameter")
    };
    assert_eq!(argument.definition, Some(location(text, "base: T", 4)));
    let property = region(&mut analysis, text, "base }");
    let observed = facts(&mut analysis, &property);
    assert_eq!(observed.properties.len(), 1);
    assert_eq!(
        observed.properties[0].definition,
        Some(location(text, "base: T", 4))
    );
    assert!(
        analysis
            .reference_index(|| false)
            .unwrap()
            .unavailable
            .is_empty()
    );
}

#[test]
fn struct_secondary_bodies_read_the_original_representation_property() {
    let text = "fun <T> pick(value: T): T = value\nstruct Pair(val value: Boolean) { constructor(seed: String): this(value = true) { val copy = pick(this.value); val label = pick(seed) } }";
    let mut analysis = analyze_construction(text);
    let body = region(&mut analysis, text, "pick(this.value)");
    let observed = facts(&mut analysis, &body);
    assert_eq!(observed.properties.len(), 1);
    assert_eq!(
        observed.properties[0].definition,
        Some(location(text, "value: Boolean", 5))
    );
    let seed = observed
        .locals
        .iter()
        .find(|local| local.reference.name == "seed")
        .unwrap();
    assert_eq!(local_type(&mut analysis, &body, seed), "String");
    let delegation = region(&mut analysis, text, "this(value = true)");
    assert_eq!(facts(&mut analysis, &delegation).callables.len(), 1);
    assert!(
        analysis
            .reference_index(|| false)
            .unwrap()
            .unavailable
            .is_empty()
    );
}

#[test]
fn inferred_initializers_reuse_their_first_source_observations() {
    let text = "fun pick(value: Boolean): Boolean = value\nclass State(seed: Boolean) { val prepared = pick(seed); fun value(): Boolean = prepared }";
    let mut analysis = analyze_construction(text);
    let getter = function(&analysis, "value");
    assert!(matches!(
        analysis.function_facts(getter, || false).unwrap(),
        V::Known(_)
    ));
    let body = region(&mut analysis, text, "pick(seed)");
    let first = facts(&mut analysis, &body);
    let count = analysis.lowerer.functions.len();
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let pick = index.targets_at(&location(text, "pick(value: Boolean)", 4))[0];
    assert_eq!(index.references_to(pick).count(), 2);
    let again = facts(&mut analysis, &body);
    assert_eq!(analysis.lowerer.functions.len(), count);
    assert_eq!(
        first.locals[0].reference.binding,
        again.locals[0].reference.binding
    );
}
