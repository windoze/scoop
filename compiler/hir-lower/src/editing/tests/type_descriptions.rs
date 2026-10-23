use super::imports::{analyze_files, at};
use super::*;

mod imported;

fn description(
    analysis: &mut DeclarationAnalysis,
    text: &str,
    needle: &str,
    name: &str,
) -> hir::EditingTypeDescription {
    let origin = at("src/types.scoop", text, needle, name.len());
    match analysis
        .type_description(&origin.source, origin.span, || false)
        .unwrap()
    {
        V::Known(Some(description)) => description,
        result => panic!(
            "missing description for {needle}: {result:?}; {:?}",
            analysis.diagnostics()
        ),
    }
}

#[test]
fn original_declarations_and_applied_aliases_survive_an_unchecked_body() {
    let text = "struct Box<元素>(val value: 元素)\ntypealias Wrapped<项目> = Box<项目>\nfun <调用> editing(value: Wrapped<调用>): Box<调用> = missing";
    let mut analysis = analyze_files(&[("src/types.scoop", text)]);
    let nominal = description(&mut analysis, text, "Box<元素>", "Box");
    assert_eq!(nominal.label, "struct Box<元素>");
    assert!(nominal.application.is_none());
    let alias = description(&mut analysis, text, "Wrapped<项目>", "Wrapped");
    assert_eq!(alias.label, "typealias Wrapped<项目> = Box<项目>");
    let applied = description(&mut analysis, text, "Wrapped<调用>", "Wrapped");
    assert_eq!(applied.target, alias.target);
    assert_eq!(applied.definition, alias.definition);
    assert_eq!(applied.application.unwrap().name, "Box<调用>");
    let parameter = description(&mut analysis, text, "调用> editing", "调用");
    assert_eq!(parameter.label, "type parameter 调用");
    assert!(parameter.application.is_none());
    assert!(analysis.body_facts.is_empty());
    assert!(analysis.reference_index.is_none());
}

#[test]
fn local_alias_applications_keep_inner_binders_after_leaving_their_scope() {
    let text = "struct Box<T>(val value: T)\nfun <Outer> editing(input: Box<Outer>): Box<Outer> { fun <Inner> nested(value: Box<Inner>): Box<Inner> { typealias Local = Box<Inner>; val copy: Local = value; return copy }; return nested(input) }";
    let mut analysis = analyze_files(&[("src/types.scoop", text)]);
    let alias = description(&mut analysis, text, "Local = value", "Local");
    assert_eq!(alias.label, "typealias Local = Box<Inner>");
    assert_eq!(alias.application.unwrap().name, "Box<Inner>");
    let inner = description(&mut analysis, text, "Inner> nested", "Inner");
    assert_eq!(inner.label, "type parameter Inner");
    let outer = description(&mut analysis, text, "Outer> editing", "Outer");
    assert_ne!(outer.target, inner.target);
    assert_eq!(outer.label, "type parameter Outer");
}

#[test]
fn exact_import_aliases_describe_the_bound_declaration_without_checking_bodies() {
    let library = "package api\nclass Outer { class Inner {}; typealias Alias<T> = T }";
    let text = "import api.Outer.Inner as Selected\nimport api.Outer.Alias as Saved\nfun broken(): Boolean = missing";
    let mut analysis = analyze_files(&[("src/api.scoop", library), ("src/types.scoop", text)]);
    let nested = description(&mut analysis, text, "Selected", "Selected");
    assert_eq!(nested.label, "class Outer.Inner");
    assert_eq!(
        nested.definition,
        Some(at("src/api.scoop", library, "Inner {}", 5))
    );
    let alias = description(&mut analysis, text, "Saved", "Saved");
    assert_eq!(alias.label, "typealias Outer.Alias<T> = T");
    assert!(alias.application.is_none());
    assert!(analysis.body_facts.is_empty());
}

#[test]
fn type_parameter_bounds_are_read_from_their_complete_declaration() {
    let text = "interface Readable {}\nfun <T: Readable> read(input: T): T = missing\nfun <T> plain(input: T): T = input";
    let mut analysis = analyze_files(&[("src/types.scoop", text)]);
    let bound = description(&mut analysis, text, "T: Readable", "T");
    assert_eq!(bound.label, "type parameter T: Readable");
    let plain = description(&mut analysis, text, "T> plain", "T");
    assert_eq!(plain.label, "type parameter T");
    assert_ne!(bound.target, plain.target);
    assert!(analysis.body_facts.is_empty());
}

#[test]
fn parameter_conditions_preserve_companions_exclusions_and_call_contracts() {
    let text = "interface Excluded {}\ninterface Maker<Item> {}\nfun <T: ref> traits(input: T): T where T: !Excluded, T.Companion: Maker<T>, requires(x: T) { @NoGC x.mark(): Boolean; x.read<String>(item = y: String): Int } = missing";
    let mut analysis = analyze_files(&[("src/types.scoop", text)]);
    let parameter = description(&mut analysis, text, "T: ref", "T");
    for expected in [
        "type parameter T: ref",
        "T: !Excluded",
        "T.Companion: Maker<T>",
        "@NoGC T.mark(): Boolean",
        "T.read<String>(item = String): Int",
    ] {
        assert!(parameter.label.contains(expected), "{}", parameter.label);
    }
    assert!(analysis.body_facts.is_empty());
}

#[test]
fn local_nominal_declarations_do_not_expose_captured_parameters_as_own_parameters() {
    let text = "fun <Outer> editing(input: Outer): Outer { class Local<Item>(val item: Item, val outer: Outer); val value = Local(true, input); return value.outer }";
    let mut analysis = analyze_files(&[("src/types.scoop", text)]);
    let local = description(&mut analysis, text, "Local<Item>", "Local");
    assert_eq!(local.label, "class Local<Item>");
}

#[test]
fn unavailable_and_cancelled_type_descriptions_do_not_publish_guesses() {
    let text =
        "class Item {}\nfun broken(input: Item): Item { val copy: Item = missing; return input }";
    let mut analysis = analyze_files(&[("src/types.scoop", text)]);
    let body = at("src/types.scoop", text, "Item = missing", 4);
    assert!(matches!(
        analysis.type_description(&body.source, body.span, || true),
        Err(EditingCancelled)
    ));
    assert!(analysis.body_facts.is_empty());
    assert!(matches!(
        analysis
            .type_description(&body.source, body.span, || false)
            .unwrap(),
        V::Unavailable(U::BodyFailure)
    ));
    assert_eq!(
        description(&mut analysis, text, "Item): Item", "Item").label,
        "class Item"
    );
    let unknown = at("src/types.scoop", text, "class", 5);
    assert!(matches!(
        analysis
            .type_description(&unknown.source, unknown.span, || false)
            .unwrap(),
        V::Known(None)
    ));
}
