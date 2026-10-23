use super::imports::{analyze_files, at};
use super::*;

mod imported;

fn argument(
    analysis: &mut DeclarationAnalysis,
    text: &str,
    needle: &str,
    name: &str,
) -> hir::EditingNamedArgument {
    let span = at("src/arguments.scoop", text, needle, name.len()).span;
    let function = function(analysis, "editing");
    match analysis.named_argument(function, span, || false).unwrap() {
        V::Known(Some(argument)) => argument,
        result => panic!(
            "missing argument for {needle}: {result:?}; {:?}",
            analysis.diagnostics()
        ),
    }
}

fn type_name(
    analysis: &mut DeclarationAnalysis,
    argument: &hir::EditingNamedArgument,
    ty: hir::TypeId,
) -> String {
    match analysis.describe_call_type(&argument.context, ty).unwrap() {
        V::Known(description) => description.name,
        result => panic!("missing applied type: {result:?}"),
    }
}

#[test]
fn named_arguments_keep_the_applied_inner_binder_and_original_parameter_owner() {
    let text = "fun <Element> echo(value: Element): Element = value\nfun <Outer> editing(input: Outer): Outer { fun <Inner> nested(item: Inner): Inner = echo(value = item); return nested(item = input) }";
    let mut analysis = analyze_files(&[("src/arguments.scoop", text)]);
    let inner = argument(&mut analysis, text, "value = item", "value");
    assert_eq!(
        type_name(&mut analysis, &inner, inner.parameter.value_type),
        "Inner"
    );
    assert_eq!(
        inner.definition,
        Some(at("src/arguments.scoop", text, "value:", 5))
    );
    assert_eq!(
        inner.callee,
        Some(hir::SemanticCallable::Current(function(&analysis, "echo")).into())
    );
    let outer = argument(&mut analysis, text, "item = input", "item");
    assert_eq!(
        type_name(&mut analysis, &outer, outer.parameter.value_type),
        "Outer"
    );
    assert_eq!(
        outer.definition,
        Some(at("src/arguments.scoop", text, "item:", 4))
    );
    assert_ne!(outer.callee, inner.callee);
    assert!(analysis.reference_index.is_none());
}

#[test]
fn named_varargs_and_constructors_use_actual_applied_parameter_types() {
    let text = "fun <T> gather(value: T, vararg rest: T): T = value\nclass Box<T>(val item: T)\nfun editing(rest: Array<Boolean>): Box<Boolean> { gather(rest = rest, value = true); return Box(item = false) }";
    let mut analysis = analyze_files(&[("src/arguments.scoop", text)]);
    let rest = argument(&mut analysis, text, "rest = rest", "rest");
    assert_eq!(rest.index, 1);
    assert_eq!(rest.parameter.name.as_deref(), Some("rest"));
    assert_eq!(
        type_name(&mut analysis, &rest, rest.parameter.value_type),
        "Array<Boolean>"
    );
    assert_eq!(
        type_name(
            &mut analysis,
            &rest,
            rest.parameter.vararg_element_type.unwrap()
        ),
        "Boolean"
    );
    let item = argument(&mut analysis, text, "item = false", "item");
    assert_eq!(item.index, 0);
    assert_eq!(
        type_name(&mut analysis, &item, item.parameter.value_type),
        "Boolean"
    );
}

#[test]
fn completed_argument_facts_survive_later_errors_but_failed_calls_are_not_published() {
    let text = "fun accept(value: Boolean) {}\nfun editing() { accept(value = true); accept(value = \"bad\"); accept(value = false) }";
    let mut analysis = analyze_files(&[("src/arguments.scoop", text)]);
    let checked = argument(&mut analysis, text, "value = true", "value");
    assert_eq!(
        type_name(&mut analysis, &checked, checked.parameter.value_type),
        "Boolean"
    );
    let function = function(&analysis, "editing");
    assert!(matches!(
        analysis.function_facts(function, || false).unwrap(),
        V::Unavailable(U::BodyFailure)
    ));
    for needle in ["value = \"bad\"", "value = false"] {
        let span = at("src/arguments.scoop", text, needle, 5).span;
        assert!(matches!(
            analysis.named_argument(function, span, || false).unwrap(),
            V::Known(None)
        ));
    }
    let span = at("src/arguments.scoop", text, "value = true", 5).span;
    assert!(matches!(
        analysis.named_argument(function, span, || true),
        Err(EditingCancelled)
    ));
}
