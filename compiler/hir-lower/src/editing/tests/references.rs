use super::*;
use hir::{EditingCallableIdentity as Callable, EditingReferenceTarget as Target};

fn origin(offset: usize, length: usize) -> hir::SemanticSourceLocation {
    hir::SemanticSourceLocation {
        source: crate::tests::core_source_identity("src/editing.scoop"),
        span: ast::Span::new(offset as u32, (offset + length) as u32),
    }
}

#[test]
fn reference_index_distinguishes_overloads_and_merges_generic_applications() {
    let text = "fun choose(value: Boolean): Boolean = value\nfun choose(value: String): Boolean = true\nfun <T> same(value: T): T = value\nfun editing(): Boolean { val first = choose(true); val second = choose(\"text\"); val third = same(\"text\"); return same(first) }";
    let mut analysis = analyze(text);
    let index = analysis.reference_index(|| false).unwrap();
    let overloads = text
        .match_indices("choose")
        .map(|(offset, _)| index.targets_at(&origin(offset, 6))[0])
        .collect::<Vec<_>>();
    assert_ne!(overloads[0], overloads[1]);
    assert_eq!(overloads[0], overloads[2]);
    assert_eq!(overloads[1], overloads[3]);
    assert_eq!(index.references_to(overloads[0]).count(), 2);
    let generics = text
        .match_indices("same")
        .map(|(offset, _)| index.targets_at(&origin(offset, 4))[0])
        .collect::<Vec<_>>();
    assert!(generics.iter().all(|target| *target == generics[0]));
    assert!(matches!(
        generics[0],
        Target::Callable(Callable::Declaration(
            hir::DefaultCallableDeclarationV1::GenericFunction(_)
        ))
    ));
    assert_eq!(
        index
            .references_to(generics[0])
            .filter(|reference| !reference.is_declaration)
            .count(),
        2
    );
}

#[test]
fn named_arguments_use_the_selected_parameter_indices() {
    let text = "fun choose(left: Boolean, right: Boolean): Boolean = left\nfun editing(): Boolean = choose(right = true, left = false)";
    let mut analysis = analyze(text);
    let index = analysis.reference_index(|| false).unwrap();
    let target = |offset, length| index.targets_at(&origin(offset, length))[0];
    let left = target(text.find("left").unwrap(), 4);
    let right = target(text.find("right").unwrap(), 5);
    assert!(matches!(left, Target::Parameter { index: 0, .. }));
    assert!(matches!(right, Target::Parameter { index: 1, .. }));
    assert_eq!(target(text.rfind("left").unwrap(), 4), left);
    assert_eq!(target(text.rfind("right").unwrap(), 5), right);
    assert_eq!(index.references_to(left).count(), 3);
    assert_eq!(index.references_to(right).count(), 2);
}

#[test]
fn local_shadowing_captures_and_named_local_functions_keep_distinct_targets() {
    let text = "fun editing(): Boolean { val value = true; if (value) { val value = false; val used = value }; fun nested(value: Boolean): Boolean = value; val reference: (Boolean) -> Boolean = ::nested; val read: () -> Boolean = { value }; return nested(value = read()) }";
    let mut analysis = analyze(text);
    let index = analysis.reference_index(|| false).unwrap();
    let outer = index.targets_at(&origin(text.find("value = true").unwrap(), 5))[0];
    let inner = index.targets_at(&origin(text.find("value = false").unwrap(), 5))[0];
    assert_ne!(outer, inner);
    assert!(matches!(outer, Target::Binding(_)));
    assert_eq!(index.references_to(outer).count(), 3);
    assert_eq!(index.references_to(inner).count(), 2);
    let nested = index.targets_at(&origin(text.find("nested").unwrap(), 6))[0];
    assert!(matches!(nested, Target::Callable(Callable::Local(_))));
    assert_eq!(index.references_to(nested).count(), 3);
    let parameter = index.targets_at(&origin(text.find("value: Boolean").unwrap(), 5))[0];
    assert!(matches!(
        parameter,
        Target::Parameter {
            callable: Callable::Local(_),
            index: 0
        }
    ));
    assert_eq!(index.references_to(parameter).count(), 3);
}

#[test]
fn property_indexes_use_logical_declarations_for_reads_and_writes() {
    let text = "class Value<T>(var item: T)\nvar <T> Value<T>.property: T get() = item\nset(value) { item = value }\nfun editing(value: Value<Boolean>): Boolean { value.property = value.item; return value.property }";
    let mut analysis = analyze(text);
    let index = analysis.reference_index(|| false).unwrap();
    let property = index.targets_at(&origin(text.find("property").unwrap(), 8))[0];
    assert!(matches!(
        property,
        Target::Property(hir::PropertyDeclarationId::ExtensionProperty(_))
    ));
    assert_eq!(index.references_to(property).count(), 3);
    let item = index.targets_at(&origin(text.find("item").unwrap(), 4))[0];
    assert!(matches!(
        item,
        Target::Property(hir::PropertyDeclarationId::Property(_))
    ));
    assert_eq!(index.references_to(item).count(), 2);
}

#[test]
fn indexes_keep_failures_explicit_and_reuse_the_same_input() {
    let text = "fun healthy(): Boolean = true\nfun broken() { missing }\nfun editing(): Boolean = healthy()";
    let mut analysis = analyze(text);
    let index = analysis.reference_index(|| false).unwrap();
    let healthy = index.targets_at(&origin(text.find("healthy").unwrap(), 7))[0];
    assert_eq!(index.references_to(healthy).count(), 2);
    assert!(
        index
            .unavailable
            .contains(&origin(text.find("broken").unwrap(), 6))
    );
    let previous = std::ptr::from_ref(index);
    let checks = std::rc::Rc::new(std::cell::Cell::new(0));
    let observed = checks.clone();
    let repeated = analysis
        .reference_index(move || {
            observed.set(observed.get() + 1);
            false
        })
        .unwrap();
    assert_eq!(std::ptr::from_ref(repeated), previous);
    assert_eq!(checks.get(), 1);
}

#[test]
fn cancelled_index_construction_is_not_published_and_can_be_retried() {
    let mut analysis = analyze("fun healthy(): Boolean = true\nfun editing(): Boolean = healthy()");
    let checks = std::rc::Rc::new(std::cell::Cell::new(0));
    let observed = checks.clone();
    assert!(matches!(
        analysis.reference_index(move || {
            observed.set(observed.get() + 1);
            observed.get() > 8
        }),
        Err(EditingCancelled)
    ));
    assert!(analysis.reference_index.is_none());
    let index = analysis.reference_index(|| false).unwrap();
    assert!(!index.occurrences().is_empty());
}
