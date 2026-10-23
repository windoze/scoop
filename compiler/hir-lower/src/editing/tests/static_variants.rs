use super::imports::{analyze_files, at};
use super::*;
use hir::{EditingCallableIdentity as Callable, EditingReferenceTarget as Target};

mod imported;

const FILE: &str = "src/static_variants.scoop";

fn variant(
    analysis: &DeclarationAnalysis,
    text: &str,
    needle: &str,
    name: &str,
) -> hir::EditingStaticVariantReference {
    let origin = at(FILE, text, needle, name.len());
    match analysis
        .static_variant_reference(&origin.source, origin.span, || false)
        .unwrap()
    {
        V::Known(Some(reference)) => reference,
        value => panic!("{needle}: {value:?}; {:?}", analysis.diagnostics()),
    }
}

fn type_name(analysis: &mut DeclarationAnalysis, ty: hir::TypeId) -> String {
    analysis
        .describe_declaration_type(
            &hir::SemanticDeclarationBinders {
                owner: Vec::new(),
                callable: Vec::new(),
            },
            ty,
        )
        .unwrap()
        .name
}

#[test]
fn static_variants_retain_original_targets_and_applied_generic_types() {
    let text = "enum Choice<T> { Empty, Item(val value: T) }\ntypealias Wrapped<U> = Choice<U>\nval first: Choice<Boolean> = Choice<Boolean>.Empty\nval second: Choice<String> = Wrapped<String>.Empty";
    let mut analysis = analyze_files(&[(FILE, text)]);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    assert_eq!(analysis.static_variant_references.len(), 2);
    let first = variant(&analysis, text, "Empty\nval", "Empty");
    let declaration = at(FILE, text, "Empty,", 5);
    assert!(matches!(
        analysis
            .static_variant_reference(&declaration.source, declaration.span, || false)
            .unwrap(),
        V::Known(None)
    ));
    let offset = text.rfind("Empty").unwrap();
    let origin = hir::SemanticSourceLocation {
        source: crate::tests::test_source_identity(FILE),
        span: ast::Span::new(offset as u32, (offset + 5) as u32),
    };
    let V::Known(Some(second)) = analysis
        .static_variant_reference(&origin.source, origin.span, || false)
        .unwrap()
    else {
        panic!("the alias initializer keeps its variant")
    };
    assert!(matches!(first.target, hir::SemanticVariant::Current(_)));
    assert_eq!(first.target, second.target);
    assert_eq!(first.name, "Empty");
    assert_eq!(first.definition, Some(at(FILE, text, "Empty,", 5)));
    assert_eq!(
        type_name(&mut analysis, first.value_type),
        "Choice<Boolean>"
    );
    assert_eq!(
        type_name(&mut analysis, second.value_type),
        "Choice<String>"
    );
    let alias = at(FILE, text, "Wrapped<String>", 7);
    let V::Known(Some(reference)) = analysis
        .type_reference(&alias.source, alias.span, || false)
        .unwrap()
    else {
        panic!("the successful trial retains its alias lookup")
    };
    assert!(matches!(
        reference.target,
        hir::EditingTypeIdentity::Alias(_)
    ));
    let V::Known(Some(ty)) = analysis
        .source_type_application(&alias.source, alias.span, || false)
        .unwrap()
    else {
        panic!("the successful trial retains its type application")
    };
    assert_eq!(type_name(&mut analysis, ty), "Choice<String>");
    assert!(analysis.body_facts.is_empty());
    assert!(analysis.lowerer.pending_runtime_initializers.is_empty());
    let index = analysis.reference_index(|| false).unwrap();
    let target = index.targets_at(&first.origin)[0];
    assert!(matches!(target, Target::Callable(Callable::Variant(_))));
    assert_eq!(index.targets_at(&second.origin), &[target]);
    assert_eq!(index.references_to(target).count(), 3);
}

#[test]
fn static_and_body_variant_uses_share_the_original_alias_and_declaration() {
    let provider = "package events\nenum Event { Empty, Other }";
    let text = "import events.Event\nimport events.Event.Empty as selected\ntypealias Events = Event\nval first: Event = selected\nval second: Event = Events.Empty\nfun read(): Event = selected";
    let mut analysis = analyze_files(&[("src/events.scoop", provider), (FILE, text)]);
    assert!(
        analysis.diagnostics().is_empty(),
        "{:?}",
        analysis.diagnostics()
    );
    let first = variant(&analysis, text, "selected\nval", "selected");
    let second = variant(&analysis, text, "Empty\nfun", "Empty");
    assert_eq!(first.target, second.target);
    let index = analysis.reference_index(|| false).unwrap();
    let alias = index.targets_at(&first.origin)[0];
    assert!(matches!(alias, Target::ImportAlias(_)));
    assert_eq!(index.references_to(alias).count(), 3);
    let original = index.targets_at(&second.origin)[0];
    assert_eq!(index.references_to(original).count(), 5);
}

#[test]
fn failed_variant_trials_do_not_leave_source_or_type_observations() {
    let text = "enum Event { Empty }\nval image: Event = Event.Empty\nval runtime: Event = if (true) Event.Empty else Event.Empty\nval wrong: String = Event.Empty";
    let mut analysis = analyze_files(&[(FILE, text)]);
    assert_eq!(analysis.static_variant_references.len(), 1);
    let failed = text.rfind("Event.Empty").unwrap();
    let source = crate::tests::test_source_identity(FILE);
    assert!(matches!(
        analysis
            .static_variant_reference(
                &source,
                ast::Span::new((failed + 6) as u32, (failed + 11) as u32),
                || false
            )
            .unwrap(),
        V::Known(None)
    ));
    assert!(matches!(
        analysis
            .type_reference(
                &source,
                ast::Span::new(failed as u32, (failed + 5) as u32),
                || false
            )
            .unwrap(),
        V::Unavailable(U::BodyFailure)
    ));
    let index = analysis.reference_index(|| false).unwrap();
    let target = index.targets_at(&at(FILE, text, "Empty }", 5))[0];
    assert_eq!(index.references_to(target).count(), 4);
    assert!(!index.unavailable.is_empty());
}

#[test]
fn static_variant_lookup_honors_cancellation_without_discarding_completed_facts() {
    let text = "enum Event { Empty }\nval image: Event = Event.Empty";
    let mut analysis = analyze_files(&[(FILE, text)]);
    let source = crate::tests::test_source_identity(FILE);
    let offset = text.rfind("Empty").unwrap() as u32;
    assert!(matches!(
        analysis.static_variant_reference(&source, ast::Span::new(offset, offset + 5), || true),
        Err(EditingCancelled)
    ));
    let V::Known(Some(reference)) = analysis
        .static_variant_reference(&source, ast::Span::new(offset, offset + 5), || false)
        .unwrap()
    else {
        panic!("the completed initializer remains available")
    };
    assert_eq!(type_name(&mut analysis, reference.value_type), "Event");
    assert!(analysis.body_facts.is_empty());
}
