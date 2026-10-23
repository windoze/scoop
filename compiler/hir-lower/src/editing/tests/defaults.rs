use super::imports::{analyze_files, at};
use super::*;
use hir::{EditingCallee as Callee, EditingReferenceTarget as Target, EditingSourceBodyId as Body};

mod contexts;
mod failures;

fn analyze_defaults(text: &str) -> DeclarationAnalysis {
    analyze_files(&[("src/defaults.scoop", text)])
}

fn location(text: &str, needle: &str, length: usize) -> hir::SemanticSourceLocation {
    at("src/defaults.scoop", text, needle, length)
}

fn region(analysis: &mut DeclarationAnalysis, text: &str, needle: &str) -> hir::EditingSourceBody {
    let location = location(text, needle, 0);
    let body = analysis
        .source_body_at(&location.source, location.span, || false)
        .unwrap()
        .unwrap_or_else(|| panic!("missing default at {needle}: {:?}", analysis.diagnostics()));
    assert!(matches!(body.id, Body::Default(_)), "{:?}", body.id);
    body
}

fn facts(
    analysis: &mut DeclarationAnalysis,
    body: &hir::EditingSourceBody,
) -> hir::EditingBodyFacts {
    match analysis.source_body_facts(body.id, || false).unwrap() {
        V::Known(facts) => facts.clone(),
        V::Unavailable(reason) => panic!(
            "default unavailable: {reason:?}; {:?}",
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
        panic!("a default retains its definition context")
    };
    description.name
}

#[test]
fn uncalled_defaults_keep_calls_arguments_and_original_parameter_targets() {
    let text = "fun choose(value: Boolean): Boolean = value\nfun never(seed: Boolean, copy: Boolean = choose(value = seed)): Boolean = copy";
    let mut analysis = analyze_defaults(text);
    assert!(analysis.body_facts.is_empty());
    let body = region(&mut analysis, text, "choose(value = seed)");
    let observed = facts(&mut analysis, &body);
    let seed = observed
        .locals
        .iter()
        .find(|local| local.reference.name == "seed" && local.role == hir::EditingLocalRole::Read)
        .unwrap();
    assert_eq!(local_type(&mut analysis, &body, seed), "Boolean");
    assert_eq!(
        seed.reference.definition,
        location(text, "seed: Boolean", 4)
    );
    assert_eq!(
        analysis.body_parameter(&body, &seed.reference.definition),
        Some((body.callee().unwrap(), 0))
    );
    let V::Known(calls) = analysis.calls(body.id, || false).unwrap() else {
        panic!("checked default call")
    };
    assert_eq!(calls.len(), 1);
    assert!(
        matches!(calls[0].context, hir::EditingCallContext::Body { body: id, .. } if id == body.id)
    );
    let V::Known(Some(argument)) = analysis
        .named_argument(body.id, location(text, "value = seed", 5).span, || false)
        .unwrap()
    else {
        panic!("checked named argument")
    };
    assert_eq!(
        argument.definition,
        Some(location(text, "value: Boolean", 5))
    );
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let target = index.targets_at(&seed.reference.definition)[0];
    assert!(matches!(target, Target::Parameter { index: 0, .. }));
    assert_eq!(index.references_to(target).count(), 2);
    let choose = index.targets_at(&location(text, "choose(value: Boolean)", 6))[0];
    assert_eq!(index.references_to(choose).count(), 2);
}

#[test]
fn default_and_caller_queries_reuse_the_first_prepared_template() {
    let text = "fun helper(value: Boolean): Boolean = value\nfun target(value: Boolean = helper(true)): Boolean = value\nfun caller(): Boolean = target()";
    for caller_first in [false, true] {
        let mut analysis = analyze_defaults(text);
        let caller = function(&analysis, "caller");
        if caller_first {
            assert!(matches!(
                analysis.function_facts(caller, || false).unwrap(),
                V::Known(_)
            ));
        }
        let body = region(&mut analysis, text, "helper(true)");
        let Body::Default(key) = body.id else {
            unreachable!()
        };
        let template = analysis.lowerer.default_templates[&key.tuple()];
        let count = analysis.lowerer.export_default_exprs.len();
        assert_eq!(facts(&mut analysis, &body).callables.len(), 1);
        assert!(matches!(
            analysis.function_facts(caller, || false).unwrap(),
            V::Known(_)
        ));
        let V::Known(calls) = analysis.calls(caller, || false).unwrap() else {
            panic!("caller checked")
        };
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].callee_span,
            Some(location(text, "target()", 6).span)
        );
        assert!(
            analysis
                .reference_index(|| false)
                .unwrap()
                .unavailable
                .is_empty()
        );
        assert_eq!(analysis.lowerer.default_templates[&key.tuple()], template);
        assert_eq!(analysis.lowerer.export_default_exprs.len(), count);
        assert!(
            analysis
                .lowerer
                .editing_defaults
                .as_ref()
                .unwrap()
                .get(&key)
                .is_none()
        );
    }
}

#[test]
fn constructor_and_variant_defaults_use_their_actual_generic_parameter_owners() {
    for text in [
        "class Box<T>(seed: T, val copy: T = seed)",
        "struct Box<T>(val seed: T, val copy: T = seed)",
        "enum Box<T> { Item(val seed: T, val copy: T = seed) }",
    ] {
        let mut analysis = analyze_defaults(text);
        let body = region(&mut analysis, text, "seed)");
        let observed = facts(&mut analysis, &body);
        let seed = observed
            .locals
            .iter()
            .find(|local| local.role == hir::EditingLocalRole::Read)
            .unwrap();
        assert_eq!(local_type(&mut analysis, &body, seed), "T");
        assert!(matches!(
            body.callee(),
            Some(Callee::Constructor(_) | Callee::CurrentVariant(_))
        ));
        assert!(analysis.source_body_signature(&body).is_none());
        assert_eq!(
            analysis.body_parameter(&body, &seed.reference.definition),
            Some((body.callee().unwrap(), 0))
        );
        let index = analysis.reference_index(|| false).unwrap();
        assert!(
            index.unavailable.is_empty(),
            "{text}: {:?}",
            index.unavailable
        );
        let target = index.targets_at(&location(text, "seed)", 4))[0];
        assert!(
            matches!(target, Target::Parameter { index: 0, .. }),
            "{text}: {target:?}"
        );
        assert_eq!(index.references_to(target).count(), 2);
    }
}

#[test]
fn recursive_cross_file_preparation_keeps_each_definition_scope_and_import_alias() {
    let api = "package api\nfun choose(value: Boolean): Boolean = value\nfun nested(value: Boolean = choose(true)): Boolean = value";
    let use_site =
        "import api.nested as fallback\nfun never(value: Boolean = fallback()): Boolean = value";
    let mut analysis = analyze_files(&[("src/api.scoop", api), ("src/use.scoop", use_site)]);
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let target = index.targets_at(&at("src/api.scoop", api, "choose(value", 6))[0];
    assert_eq!(index.references_to(target).count(), 2);
    let alias = index.targets_at(&at("src/use.scoop", use_site, "fallback()", 8))[0];
    assert!(matches!(alias, Target::ImportAlias(_)));
    assert_eq!(index.references_to(alias).count(), 2);
}
