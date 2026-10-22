use super::imports::{analyze_files, at};
use super::*;

mod imported;

fn declaration(
    analysis: &mut DeclarationAnalysis,
    text: &str,
    needle: &str,
    name: &str,
) -> hir::EditingPropertyDeclaration {
    let origin = at("src/properties.scoop", text, needle, name.len());
    match analysis
        .property_declaration(&origin.source, origin.span, || false)
        .unwrap()
    {
        V::Known(Some(declaration)) => declaration,
        result => panic!(
            "missing declaration for {needle}: {result:?}; {:?}",
            analysis.diagnostics()
        ),
    }
}

fn type_name(
    analysis: &mut DeclarationAnalysis,
    declaration: &hir::EditingPropertyDeclaration,
) -> String {
    analysis
        .describe_declaration_type(&declaration.binders, declaration.value_type)
        .unwrap()
        .name
}

#[test]
fn declarations_keep_original_binders_without_checking_unrelated_bodies() {
    let text = "val top: Boolean = true\nclass Box<Element>(var item: Element)\nvar <Item> Box<Item>.projected: Item get() = item\nset(value) { item = value }\nobject Values { val flag: Boolean get() = missing }\ninterface Value { val abstracted: Boolean }\nfun broken(): Boolean = missing";
    let mut analysis = analyze_files(&[("src/properties.scoop", text)]);
    let top = declaration(&mut analysis, text, "top:", "top");
    assert_eq!(type_name(&mut analysis, &top), "Boolean");
    assert!(!top.mutable);
    assert!(top.receiver.is_none());
    assert!(top.binders.owner.is_empty());
    let item = declaration(&mut analysis, text, "item:", "item");
    assert_eq!(type_name(&mut analysis, &item), "Element");
    assert!(item.mutable);
    assert_eq!(item.binders.owner[0].name, "Element");
    assert!(item.binders.callable.is_empty());
    let extension = declaration(&mut analysis, text, "projected:", "projected");
    assert_eq!(type_name(&mut analysis, &extension), "Item");
    assert!(extension.binders.owner.is_empty());
    assert_eq!(extension.binders.callable[0].name, "Item");
    assert_eq!(
        analysis
            .describe_declaration_type(&extension.binders, extension.receiver.unwrap())
            .unwrap()
            .name,
        "Box<Item>"
    );
    assert_ne!(extension.property, item.property);
    for name in ["flag", "abstracted"] {
        let property = declaration(&mut analysis, text, name, name);
        assert_eq!(type_name(&mut analysis, &property), "Boolean");
    }
    assert!(analysis.body_facts.is_empty());
    assert!(analysis.reference_index.is_none());
}

#[test]
fn exact_import_aliases_describe_the_original_logical_property() {
    let provider =
        "package api\nvar state: Boolean = true\nobject Values { val flag: Boolean = true }";
    let text = "import api.state as selected\nimport api.Values.flag as saved\nfun broken(): Boolean = missing";
    let mut analysis =
        analyze_files(&[("src/api.scoop", provider), ("src/properties.scoop", text)]);
    let original = declaration(&mut analysis, text, "state", "state");
    let alias = declaration(&mut analysis, text, "selected", "selected");
    assert_eq!(original.property, alias.property);
    assert_eq!(alias.name, "state");
    assert_eq!(
        alias.origin,
        Some(at("src/api.scoop", provider, "state", 5))
    );
    let member = declaration(&mut analysis, text, "saved", "saved");
    assert_eq!(member.name, "flag");
    assert!(!member.mutable);
    assert!(analysis.body_facts.is_empty());
}

#[test]
fn local_properties_keep_captured_binders_only_when_their_body_is_available() {
    let text = "fun <Outer> editing(input: Outer): Outer { class Local<Item>(val outer: Outer, val item: Item); return Local(input, true).outer }\nfun broken(): Boolean { class Damaged(val hidden: Boolean); return missing }";
    let mut analysis = analyze_files(&[("src/properties.scoop", text)]);
    let outer = declaration(&mut analysis, text, "outer:", "outer");
    assert_eq!(type_name(&mut analysis, &outer), "Outer");
    let item = declaration(&mut analysis, text, "item:", "item");
    assert_eq!(type_name(&mut analysis, &item), "Item");
    let hidden = at("src/properties.scoop", text, "hidden:", 6);
    assert!(matches!(
        analysis
            .property_declaration(&hidden.source, hidden.span, || false)
            .unwrap(),
        V::Unavailable(U::BodyFailure)
    ));
}

#[test]
fn failed_inference_and_cancellation_do_not_publish_placeholder_types() {
    let text = "val pending = missing\nval known: Boolean = true";
    let mut analysis = analyze_files(&[("src/properties.scoop", text)]);
    let pending = at("src/properties.scoop", text, "pending", 7);
    assert!(matches!(
        analysis.property_declaration(&pending.source, pending.span, || true),
        Err(EditingCancelled)
    ));
    assert!(matches!(
        analysis
            .property_declaration(&pending.source, pending.span, || false)
            .unwrap(),
        V::Unavailable(U::PendingInference)
    ));
    let known = declaration(&mut analysis, text, "known", "known");
    assert_eq!(type_name(&mut analysis, &known), "Boolean");
    assert!(analysis.body_facts.is_empty());
}
