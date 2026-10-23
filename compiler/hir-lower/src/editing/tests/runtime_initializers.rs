use super::imports::{analyze_files, at};
use super::*;
use hir::{EditingReferenceTarget as Target, EditingSourceBodyOwner as Owner};

mod contexts;
mod failures;

fn analyze_initializers(text: &str) -> DeclarationAnalysis {
    analyze_files(&[("src/initializers.scoop", text)])
}

fn region(analysis: &mut DeclarationAnalysis, text: &str, needle: &str) -> hir::EditingSourceBody {
    let location = at("src/initializers.scoop", text, needle, needle.len());
    analysis
        .source_body_at(&location.source, location.span, || false)
        .unwrap()
        .unwrap_or_else(|| {
            panic!(
                "missing initializer at {needle}: {:?}",
                analysis.diagnostics()
            )
        })
}

fn facts(
    analysis: &mut DeclarationAnalysis,
    region: &hir::EditingSourceBody,
) -> hir::EditingBodyFacts {
    match analysis.function_facts(region.function, || false).unwrap() {
        V::Known(facts) => facts.clone(),
        V::Unavailable(reason) => panic!(
            "initializer unavailable: {reason:?}; {:?}",
            analysis.diagnostics()
        ),
    }
}

fn type_name(
    analysis: &mut DeclarationAnalysis,
    region: &hir::EditingSourceBody,
    local: &hir::EditingLocalOccurrence,
) -> String {
    let V::Known(description) = analysis
        .describe_body_type(region.function, local.context, local.value_type)
        .unwrap()
    else {
        panic!("a checked initializer local keeps its type context")
    };
    description.name
}

#[test]
fn runtime_initializers_keep_real_units_calls_and_property_references() {
    let text = "fun pass(value: Boolean): Boolean = value\nval input: Boolean = true\nval result: Boolean = pass(value = input)";
    let mut analysis = analyze_initializers(text);
    let body = region(&mut analysis, text, "pass(value = input)");
    let Owner::Initialization(unit) = body.owner else {
        panic!("actual initialization unit")
    };
    assert_eq!(
        analysis.lowerer.initialization_units[unit].initializer,
        body.function
    );
    assert_eq!(
        body.origin,
        at("src/initializers.scoop", text, "pass(value = input)", 19)
    );
    assert!(body.callable().is_none());
    assert_eq!(analysis.functions().len(), 1);
    let facts = facts(&mut analysis, &body);
    assert_eq!(facts.callables.len(), 1);
    assert_eq!(facts.properties.len(), 1);
    let argument = at("src/initializers.scoop", text, "value = input", 5);
    let V::Known(Some(argument)) = analysis
        .named_argument(body.function, argument.span, || false)
        .unwrap()
    else {
        panic!("a checked initializer call has its real argument map")
    };
    assert_eq!(
        argument.definition,
        Some(at("src/initializers.scoop", text, "value: Boolean", 5))
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let input = at("src/initializers.scoop", text, "input:", 5);
    let target = index.targets_at(&input)[0];
    assert!(matches!(target, Target::Property(_)));
    assert_eq!(index.references_to(target).count(), 2);
}

#[test]
fn previously_inferred_initializers_use_the_current_property_type() {
    let text = "fun pass(value: Boolean): Boolean = value\nval result = pass(value = true)\nfun editing(): Boolean = result";
    for query_caller_first in [false, true] {
        let mut analysis = analyze_initializers(text);
        if query_caller_first {
            let function = function(&analysis, "editing");
            assert!(matches!(
                analysis.function_facts(function, || false).unwrap(),
                V::Known(_)
            ));
        }
        let body = region(&mut analysis, text, "pass(value = true)");
        assert_eq!(facts(&mut analysis, &body).callables.len(), 1);
        let name = at("src/initializers.scoop", text, "result =", 6);
        let V::Known(Some(property)) = analysis
            .property_declaration(&name.source, name.span, || false)
            .unwrap()
        else {
            panic!("the ordinary inference query completed the property")
        };
        assert_eq!(
            analysis
                .describe_declaration_type(&property.binders, property.value_type)
                .unwrap()
                .name,
            "Boolean"
        );
        assert!(
            analysis.diagnostics().is_empty(),
            "{:?}",
            analysis.diagnostics()
        );
    }
}

#[test]
fn delegated_initializers_keep_source_calls_without_inventing_protocol_name_tokens() {
    let text = "class Slot(val value: Boolean) { operator fun provideDelegate(): Slot = this\noperator fun getValue(thisRef: Unit): Boolean = value }\nfun pass(value: Boolean): Boolean = value\nval delegated by Slot(value = pass(true))";
    let mut analysis = analyze_initializers(text);
    let body = region(&mut analysis, text, "Slot(value = pass(true))");
    let facts = facts(&mut analysis, &body);
    assert_eq!(facts.callables.len(), 2, "{:?}", facts.callables);
    for reference in &facts.callables {
        let name = &text[reference.span.start as usize..reference.span.end as usize];
        assert!(matches!(name, "Slot" | "pass"), "{name}");
    }
    assert!(
        analysis
            .reference_index(|| false)
            .unwrap()
            .unavailable
            .is_empty()
    );
}
