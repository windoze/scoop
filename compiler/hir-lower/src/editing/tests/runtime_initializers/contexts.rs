use super::*;

#[test]
fn initializer_locals_local_functions_and_captures_share_the_original_definition_root() {
    let text = "val result: Boolean = if (true) { val seed = true; val read: () -> Boolean = { seed }; fun <T> same(value: T): T = value; same(read()) } else false";
    let mut analysis = analyze_initializers(text);
    let body = region(&mut analysis, text, "same(read())");
    let facts = facts(&mut analysis, &body);
    let seed = facts
        .locals
        .iter()
        .find(|local| local.reference.name == "seed" && local.role == hir::EditingLocalRole::Read)
        .unwrap();
    assert_eq!(type_name(&mut analysis, &body, seed), "Boolean");
    assert_eq!(
        seed.reference.definition,
        at("src/initializers.scoop", text, "seed =", 4)
    );
    let parameter = facts
        .locals
        .iter()
        .find(|local| local.reference.name == "value" && local.role == hir::EditingLocalRole::Read)
        .unwrap();
    assert_eq!(type_name(&mut analysis, &body, parameter), "T");
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    let declaration = at("src/initializers.scoop", text, "same(value:", 4);
    let target = index.targets_at(&declaration)[0];
    assert_eq!(index.references_to(target).count(), 2);
}

#[test]
fn local_type_accessors_are_selected_after_their_initializer_scope() {
    let text = "val result: Boolean = if (true) { class Local(val item: Boolean) { val copied: Boolean get() { val saved = item; return saved } }; Local(true).copied } else false";
    let mut analysis = analyze_initializers(text);
    let body = region(&mut analysis, text, "return saved");
    assert!(matches!(
        body.callable(),
        Some(hir::SemanticCallable::CurrentGetter(_))
    ));
    let facts = facts(&mut analysis, &body);
    let saved = facts
        .locals
        .iter()
        .find(|local| local.reference.name == "saved" && local.role == hir::EditingLocalRole::Read)
        .unwrap();
    assert_eq!(type_name(&mut analysis, &body, saved), "Boolean");
    let index = analysis.reference_index(|| false).unwrap();
    assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
    assert_eq!(
        index
            .references_to(index.targets_at(&saved.reference.definition)[0])
            .count(),
        2
    );
}

#[test]
fn generic_extension_delegate_initializers_preserve_the_declaring_binder_without_a_receiver_value()
{
    let text = "class Box<T>(val item: T)\nclass Slot<T> { operator fun getValue(thisRef: Box<T>): T = thisRef.item }\nval <T> Box<T>.projected: T by if (true) { fun same(value: T): T = value; val read: (T) -> T = ::same; Slot<T>() } else Slot<T>()";
    let mut analysis = analyze_initializers(text);
    let body = region(&mut analysis, text, "::same");
    assert!(matches!(body.owner, Owner::Initialization(_)));
    let facts = facts(&mut analysis, &body);
    let value = facts
        .locals
        .iter()
        .find(|local| local.reference.name == "value" && local.role == hir::EditingLocalRole::Read)
        .unwrap();
    assert_eq!(type_name(&mut analysis, &body, value), "T");
    assert!(
        analysis
            .reference_index(|| false)
            .unwrap()
            .unavailable
            .is_empty()
    );
}
