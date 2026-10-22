use super::imports::{analyze_files, at};
use super::*;

fn application(
    analysis: &mut DeclarationAnalysis,
    text: &str,
    needle: &str,
    length: usize,
) -> hir::TypeId {
    let origin = at("src/types.scoop", text, needle, length);
    match analysis
        .source_type_application(&origin.source, origin.span, || false)
        .unwrap()
    {
        V::Known(Some(ty)) => ty,
        result => panic!(
            "missing application {needle}: {result:?}; {:?}",
            analysis.diagnostics()
        ),
    }
}

fn assert_origin(analysis: &mut DeclarationAnalysis, ty: hir::TypeId, text: &str, expected: &str) {
    let origin = analysis
        .type_definition(ty)
        .unwrap()
        .expect("source declaration");
    assert_eq!(
        origin.source,
        crate::tests::test_source_identity("src/types.scoop")
    );
    assert_eq!(
        &text[origin.span.start as usize..origin.span.end as usize],
        expected
    );
}

#[test]
fn aliases_keep_complete_applications_without_changing_name_definitions() {
    let text = "class Item {}\ntypealias Same<T> = T\ntypealias Saved = Item\nfun editing(value: Same<Item>): Saved = value";
    let mut analysis = analyze_files(&[("src/types.scoop", text)]);
    let use_type = application(&mut analysis, text, "Same<Item>", 4);
    assert_origin(&mut analysis, use_type, text, "class Item {}");
    let declaration = application(&mut analysis, text, "Saved =", 5);
    assert_origin(&mut analysis, declaration, text, "class Item {}");
    let generic = application(&mut analysis, text, "Same<T>", 4);
    assert_origin(&mut analysis, generic, text, "T");
    let origin = at("src/types.scoop", text, "Same<Item>", 4);
    let V::Known(Some(reference)) = analysis
        .type_reference(&origin.source, origin.span, || false)
        .unwrap()
    else {
        panic!("the alias name remains bound");
    };
    assert!(matches!(
        reference.target,
        hir::EditingTypeIdentity::Alias(_)
    ));
    assert_eq!(
        reference.definition.unwrap(),
        at("src/types.scoop", text, "Same<T>", 4)
    );
    assert!(analysis.body_facts.is_empty());
}

#[test]
fn generic_values_and_call_results_use_the_original_parameter_or_applied_nominal() {
    let text = "class Item {}\nfun <T> same(value: T): T = value\nfun editing(value: Item): Item = same(value)";
    let mut analysis = analyze_files(&[("src/types.scoop", text)]);
    let parameter = application(&mut analysis, text, "T):", 1);
    assert_origin(&mut analysis, parameter, text, "T");
    let same = function(&analysis, "same");
    let V::Known(signature) = analysis.signature(same) else {
        panic!("generic signature")
    };
    assert_eq!(
        analysis.type_definition(parameter).unwrap().unwrap().span,
        signature.type_parameters[0].span
    );
    let editing = function(&analysis, "editing");
    let V::Known(calls) = analysis.calls(editing, || false).unwrap() else {
        panic!("completed call")
    };
    let call = calls[0].clone();
    assert_eq!(
        call.callee_span,
        Some(at("src/types.scoop", text, "same(value)", 4).span)
    );
    assert_origin(&mut analysis, call.result, text, "class Item {}");
}

#[test]
fn structural_types_have_no_declaration_and_failed_applications_stay_unavailable() {
    let text = "class Item {}\nfun editing(tuple: (Item, Item), callback: () -> Item): Item = callback()\nfun damaged(input: Item): Item { val copy: Item = missing; return input }\nfun recovery(input: Item) { input. }";
    let mut analysis = analyze_files(&[("src/types.scoop", text)]);
    let editing = function(&analysis, "editing");
    let V::Known(signature) = analysis.signature(editing) else {
        panic!("complete signature")
    };
    for parameter in signature.parameters {
        assert!(
            analysis
                .type_definition(parameter.value_type)
                .unwrap()
                .is_none()
        );
    }
    let V::Known(calls) = analysis.calls(editing, || false).unwrap() else {
        panic!("completed invocation")
    };
    let result = calls[0].result;
    assert_origin(&mut analysis, result, text, "class Item {}");
    let bad = at("src/types.scoop", text, "Item = missing", 4);
    assert!(matches!(
        analysis
            .source_type_application(&bad.source, bad.span, || false)
            .unwrap(),
        V::Unavailable(U::BodyFailure)
    ));
    let recovery = function(&analysis, "recovery");
    let receivers = analysis.receivers(recovery, || false).unwrap();
    let V::Known(ty) = receivers[0].value_type else {
        panic!("known receiver type")
    };
    assert_origin(&mut analysis, ty, text, "class Item {}");
    assert!(matches!(
        analysis.source_type_application(&bad.source, bad.span, || true),
        Err(EditingCancelled)
    ));
}

#[test]
fn open_nominal_types_keep_their_source_with_a_generic_companion() {
    let text = "struct Box<T>(val value: T) { companion object {} }\nfun <T> editing(value: Box<T>): Box<T> = value";
    let mut analysis = analyze_files(&[("src/types.scoop", text)]);
    let editing = function(&analysis, "editing");
    let V::Known(signature) = analysis.signature(editing) else {
        panic!("complete signature")
    };
    assert_origin(
        &mut analysis,
        signature.parameters[0].value_type,
        text,
        "struct Box<T>(val value: T) { companion object {} }",
    );
    let V::Known(facts) = analysis.function_facts(editing, || false).unwrap() else {
        panic!("complete body")
    };
    let ty = facts
        .locals
        .iter()
        .find(|local| local.role == hir::EditingLocalRole::Read)
        .unwrap()
        .value_type;
    assert_origin(
        &mut analysis,
        ty,
        text,
        "struct Box<T>(val value: T) { companion object {} }",
    );
}
