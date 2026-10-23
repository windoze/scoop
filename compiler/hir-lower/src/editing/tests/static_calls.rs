use super::imports::{analyze_files, at};
use super::*;
use hir::EditingReferenceTarget as Target;

const FILE: &str = "src/static_calls.scoop";

fn calls(analysis: &DeclarationAnalysis) -> Vec<hir::EditingAppliedCall> {
    let source = crate::tests::test_source_identity(FILE);
    match analysis.static_calls(&source, || false).unwrap() {
        V::Known(calls) => calls,
        result => panic!("{result:?}; {:?}", analysis.diagnostics()),
    }
}

fn type_name(
    analysis: &mut DeclarationAnalysis,
    call: &hir::EditingAppliedCall,
    ty: hir::TypeId,
) -> String {
    match analysis.describe_call_type(&call.context, ty).unwrap() {
        V::Known(description) => description.name,
        result => panic!("{result:?}"),
    }
}

#[test]
fn constants_retain_selected_intrinsics_and_actual_named_parameter_types() {
    let text = "const val shifted: Int = 3.shl(count = 1L)\nconst val infixValue: Int = 4 shl 2L\nval image: Int = 5.shl(count = 3L)\nfun runtime(value: Int): Int = value.shl(count = 1L)";
    let mut analysis = analyze_files(&[(FILE, text)]);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    let calls = calls(&analysis);
    assert_eq!(calls.len(), 3);
    assert!(analysis.body_facts.is_empty());
    assert!(analysis.lowerer.pending_runtime_initializers.is_empty());
    for call in &calls {
        assert!(matches!(
            call.context,
            hir::EditingCallContext::Declaration(_)
        ));
        assert!(matches!(
            call.callee,
            Some(hir::EditingCallee::Callable(
                hir::SemanticCallable::Imported(_)
            ))
        ));
        assert_eq!(call.callee, calls[0].callee);
        assert_eq!(call.parameters[0].name.as_deref(), Some("count"));
        assert_eq!(
            type_name(&mut analysis, call, call.parameters[0].value_type),
            "Long"
        );
        assert_eq!(type_name(&mut analysis, call, call.result), "Int");
        assert_eq!(call.arguments[0].parameter, 0);
    }
    let origin = at(FILE, text, "count = 1L", 5);
    let V::Known(Some(argument)) = analysis
        .static_named_argument(&origin.source, origin.span, || false)
        .unwrap()
    else {
        panic!("the original named parameter is available")
    };
    assert_eq!(argument.index, 0);
    assert_eq!(argument.callee, calls[0].callee);
    assert_eq!(
        argument.definition.unwrap().source.cone(),
        ConeIdentity::CORE
    );
    let index = analysis.reference_index(|| false).unwrap();
    let target = index.targets_at(&at(FILE, text, "shl(count = 1L)", 3))[0];
    assert!(matches!(target, Target::Callable(_)));
    assert_eq!(index.references_to(target).count(), 4);
    let parameter = index.targets_at(&origin)[0];
    assert!(matches!(parameter, Target::Parameter { index: 0, .. }));
    assert_eq!(index.references_to(parameter).count(), 3);
}

#[test]
fn nested_integer_float_and_conversion_calls_keep_each_checked_result() {
    let text = "const val converted: Double = 2.plus(other = 3).toDouble()\nconst val fraction: Double = (1.5).plus(other = 2.5)\nval quotient: Double = (8.0).div(other = 2.0)\nval rounded: Int = (3.5).toInt()\nconst val divided: Int = 9.div(other = 3)\nval negative: Int = 4.unaryMinus()";
    let mut analysis = analyze_files(&[(FILE, text)]);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    let calls = calls(&analysis);
    assert_eq!(calls.len(), 7);
    let mut signatures = calls
        .iter()
        .map(|call| {
            let parameters = call
                .parameters
                .iter()
                .map(|parameter| type_name(&mut analysis, call, parameter.value_type))
                .collect::<Vec<_>>();
            (
                call.name.clone().unwrap(),
                parameters,
                type_name(&mut analysis, call, call.result),
            )
        })
        .collect::<Vec<_>>();
    signatures.sort();
    assert_eq!(
        signatures,
        [
            ("div".into(), vec!["Double".into()], "Double".into()),
            ("div".into(), vec!["Int".into()], "Int".into()),
            ("plus".into(), vec!["Double".into()], "Double".into()),
            ("plus".into(), vec!["Int".into()], "Int".into()),
            ("toDouble".into(), vec![], "Double".into()),
            ("toInt".into(), vec![], "Int".into()),
            ("unaryMinus".into(), vec![], "Int".into()),
        ]
    );
    assert!(analysis.body_facts.is_empty());
}

#[test]
fn failed_constants_and_static_probes_do_not_duplicate_runtime_calls() {
    let text = "const val badType: String = 2.plus(other = 3)\nconst val badDivision: Int = 3.div(other = 0)\nfun source(): Int = 1\nval runtime: Int = 4.plus(other = 5) + source()\nval wrong: String = 6.plus(other = 7)\nval healthy: Int = 8.plus(other = 9)";
    let mut analysis = analyze_files(&[(FILE, text)]);
    assert!(!analysis.diagnostics().is_empty());
    let static_calls = calls(&analysis);
    assert_eq!(static_calls.len(), 1);
    assert_eq!(
        static_calls[0].callee_span,
        Some(at(FILE, text, "plus(other = 9)", 4).span)
    );
    let index = analysis.reference_index(|| false).unwrap();
    let target = index.targets_at(&at(FILE, text, "plus(other = 9)", 4))[0];
    let mut uses = index
        .references_to(target)
        .map(|reference| reference.origin.clone())
        .collect::<Vec<_>>();
    uses.sort_by_key(|origin| origin.span.start);
    assert_eq!(
        uses,
        [
            at(FILE, text, "plus(other = 5)", 4),
            at(FILE, text, "+ source()", 1),
            at(FILE, text, "plus(other = 9)", 4),
        ]
    );
    for needle in ["plus(other = 3)", "div(other = 0)", "plus(other = 7)"] {
        assert!(
            index
                .targets_at(&at(FILE, text, needle, needle.find('(').unwrap()))
                .is_empty()
        );
    }
}

#[test]
fn recursive_constant_evaluation_retains_the_original_file_once() {
    let first = "const val result: Int = later.plus(other = 1)";
    let second = "const val later: Int = 2.shl(count = 1L)";
    let analysis = analyze_files(&[(FILE, first), ("src/later.scoop", second)]);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    assert_eq!(analysis.static_calls.len(), 2);
    assert_eq!(calls(&analysis)[0].name.as_deref(), Some("plus"));
    let origin = at("src/later.scoop", second, "shl(count", 3);
    let V::Known(Some(reference)) = analysis
        .static_callable_reference(&origin.source, origin.span, || false)
        .unwrap()
    else {
        panic!("the recursively evaluated call retains its own source")
    };
    assert_eq!(reference.span, origin.span);
}

#[test]
fn current_core_calls_keep_the_actual_function_without_a_synthetic_body() {
    let text =
        "const val number: Int = 3.shl(count = 1L)\nval floating: Double = (2.0).plus(other = 1.0)";
    let analysis = analyze(text);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    let source = crate::tests::core_source_identity("src/editing.scoop");
    let V::Known(calls) = analysis.static_calls(&source, || false).unwrap() else {
        panic!("current core calls are available")
    };
    assert_eq!(calls.len(), 2);
    for call in calls {
        let Some(hir::EditingCallee::Callable(hir::SemanticCallable::Current(function))) =
            call.callee
        else {
            panic!("the selected source function is retained")
        };
        assert_eq!(
            analysis.lowerer.source_function_name(function),
            call.name.as_deref().unwrap()
        );
    }
    assert!(analysis.body_facts.is_empty());
}

#[test]
fn cancellation_leaves_completed_static_facts_available() {
    let text = "const val value: Int = 3.shl(count = 1L)";
    let analysis = analyze_files(&[(FILE, text)]);
    let source = crate::tests::test_source_identity(FILE);
    assert!(matches!(
        analysis.static_calls(&source, || true),
        Err(EditingCancelled)
    ));
    let origin = at(FILE, text, "count", 5);
    assert!(matches!(
        analysis.static_named_argument(&source, origin.span, || true),
        Err(EditingCancelled)
    ));
    assert_eq!(calls(&analysis).len(), 1);
}

#[test]
fn named_infix_syntax_shares_the_original_target_with_body_calls() {
    let text = "const val mask: Int = 2 shl 1L\nfun editing(value: Int): Int = value shl 1L";
    let mut analysis = analyze_files(&[(FILE, text)]);
    let target = calls(&analysis)[0].callee;
    let function = function(&analysis, "editing");
    let V::Known(calls) = analysis.calls(function, || false).unwrap() else {
        panic!("the complete infix body is available")
    };
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].callee, target);
    assert_eq!(calls[0].arguments[0].parameter, 0);
    let index = analysis.reference_index(|| false).unwrap();
    let target = index.targets_at(&at(FILE, text, "shl 1L", 3))[0];
    assert_eq!(index.references_to(target).count(), 2);
}
