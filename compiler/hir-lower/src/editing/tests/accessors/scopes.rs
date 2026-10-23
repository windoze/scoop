use super::*;

#[test]
fn member_and_extension_bodies_preserve_generic_receivers_and_capture_contexts() {
    for text in [
        "class Box<T>(var item: T) { var projected: T get() { val saved = item; val read: () -> T = { saved }; return read() }\nset(next) { item = next } }",
        "class Box<T>(var item: T)\nvar <T> Box<T>.projected: T get() { val saved = item; val read: () -> T = { saved }; return read() }\nset(next) { item = next }",
    ] {
        let mut analysis = analyze_accessors(text);
        let getter = region(&mut analysis, text, "saved };");
        let body = facts(&mut analysis, &getter);
        let saved = body
            .locals
            .iter()
            .find(|local| {
                local.reference.name == "saved" && local.role == hir::EditingLocalRole::Read
            })
            .unwrap();
        assert_eq!(local_type(&mut analysis, &getter, saved), "T", "{text}");
        assert_eq!(body.properties.len(), 1);
        assert_eq!(body.properties[0].access, hir::EditingPropertyAccess::Read);
        let setter = region(&mut analysis, text, "item = next");
        let body = facts(&mut analysis, &setter);
        let next = body
            .locals
            .iter()
            .find(|local| {
                local.reference.name == "next" && local.role == hir::EditingLocalRole::Read
            })
            .unwrap();
        assert_eq!(local_type(&mut analysis, &setter, next), "T", "{text}");
        assert_eq!(body.properties.len(), 1);
        assert_eq!(body.properties[0].access, hir::EditingPropertyAccess::Write);
        let index = analysis.reference_index(|| false).unwrap();
        assert!(
            index.unavailable.is_empty(),
            "{text}: {:?}",
            index.unavailable
        );
    }
}

#[test]
fn nested_local_accessors_obtain_their_complete_outer_source_contexts() {
    let text = "fun <Outer> editing(input: Outer): Outer { class Local<T>(val item: T) { val projected: T get() { class Nested<U>(val stored: U) { val result: U get() { val saved = stored; return saved } }; return Nested(item).result } }; return Local(input).projected }";
    let mut analysis = analyze_accessors(text);
    assert!(
        analysis
            .source_bodies()
            .iter()
            .all(|body| matches!(body.callable, hir::SemanticCallable::Current(_)))
    );
    let nested = region(&mut analysis, text, "return saved");
    assert!(matches!(
        nested.callable,
        hir::SemanticCallable::CurrentGetter(_)
    ));
    let body = facts(&mut analysis, &nested);
    let saved = body
        .locals
        .iter()
        .find(|local| local.reference.name == "saved" && local.role == hir::EditingLocalRole::Read)
        .unwrap();
    assert_eq!(local_type(&mut analysis, &nested, saved), "U");
    assert_eq!(analysis.source_bodies().len(), 3);
    let property = at("src/accessors.scoop", text, "result:", 6);
    assert!(matches!(
        analysis
            .property_declaration(&property.source, property.span, || false)
            .unwrap(),
        V::Known(Some(_))
    ));
    let annotation = at("src/accessors.scoop", text, "U get()", 1);
    assert!(matches!(
        analysis
            .type_reference(&annotation.source, annotation.span, || false)
            .unwrap(),
        V::Known(Some(_))
    ));
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let parameter = at("src/accessors.scoop", text, "stored:", 6);
    assert!(!index.targets_at(&parameter).is_empty());
}

#[test]
fn interface_default_accessors_query_the_original_declaring_receiver() {
    let text = "interface Value { val item: Boolean\nvar projected: Boolean get() { val saved = item; return saved }\nset(next) { val saved = next } }\nclass Concrete(override val item: Boolean): Value";
    let mut analysis = analyze_accessors(text);
    let getter = region(&mut analysis, text, "return saved");
    let body = facts(&mut analysis, &getter);
    assert_eq!(body.properties.len(), 1);
    let setter = region(&mut analysis, text, "saved = next");
    let body = facts(&mut analysis, &setter);
    let next = body
        .locals
        .iter()
        .find(|local| local.reference.name == "next" && local.role == hir::EditingLocalRole::Read)
        .unwrap();
    assert_eq!(local_type(&mut analysis, &setter, next), "Boolean");
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
}
