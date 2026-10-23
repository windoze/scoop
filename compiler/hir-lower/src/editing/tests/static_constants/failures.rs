use super::*;

#[test]
fn failed_and_cyclic_constants_do_not_publish_partial_reads() {
    for text in [
        "const val bad = good && missing\nconst val good: Boolean = true",
        "const val bad: String = good\nconst val good: Boolean = true",
        "const val first: Boolean = second\nconst val second: Boolean = first",
    ] {
        let mut analysis = analyze_files(&[(FILE, text)]);
        assert!(!analysis.diagnostics().is_empty());
        assert!(analysis.constant_references.is_empty());
        let name = if text.contains("bad") { "bad" } else { "first" };
        let declaration = at(FILE, text, name, name.len());
        assert!(!matches!(
            analysis
                .property_declaration(&declaration.source, declaration.span, || false)
                .unwrap(),
            V::Known(Some(_))
        ));
    }
}

#[test]
fn constant_reference_queries_honor_cancellation_and_reuse_the_same_input() {
    let text =
        "const val first: Boolean = true\nconst val second = first\nval image: Boolean = first";
    let mut analysis = analyze_files(&[(FILE, text)]);
    let reference = at(FILE, text, "first\nval", 5);
    assert!(matches!(
        analysis.property_declaration(&reference.source, reference.span, || true),
        Err(EditingCancelled)
    ));
    let checks = std::rc::Rc::new(std::cell::Cell::new(0));
    let observed = checks.clone();
    assert!(matches!(
        analysis.reference_index(move || {
            observed.set(observed.get() + 1);
            observed.get() > 2
        }),
        Err(EditingCancelled)
    ));
    assert!(analysis.reference_index.is_none());
    let index = analysis.reference_index(|| false).unwrap();
    let target = index.targets_at(&reference)[0];
    assert_eq!(index.references_to(target).count(), 3);
    let previous = std::ptr::from_ref(index);
    assert_eq!(
        previous,
        std::ptr::from_ref(analysis.reference_index(|| false).unwrap())
    );
    assert!(analysis.body_facts.is_empty());
}
