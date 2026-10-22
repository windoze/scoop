use scoop_analysis::AnalysisHost;
use scoop_source::{SourceText, Span};

#[test]
fn concrete_grammar_repairs_preserve_source_versions_and_form_valid_syntax() {
    let mut host = AnalysisHost::default();
    let text = "fun `😀名`(value: Int other: Int { return value }";
    let id = host.add_source(SourceText::from(text.to_owned()));
    let source = host.source(id).unwrap();
    let fixes = source
        .syntax_fixes(Span::new(0, text.len() as u32))
        .unwrap();
    assert_eq!(fixes.len(), 2);
    assert_eq!(fixes[0].edit.replacement, ",");
    assert_eq!(fixes[1].edit.replacement, ")");
    let mut repaired = text.to_owned();
    for fix in fixes.iter().rev() {
        assert_eq!(fix.edit.source, source.version);
        assert_eq!(source.syntax_fix(fix.missing).unwrap().as_ref(), Some(fix));
        repaired.replace_range(
            fix.edit.span.start as usize..fix.edit.span.end as usize,
            &fix.edit.replacement,
        );
    }
    let first = fixes[0].edit.span;
    assert_eq!(source.syntax_fixes(first).unwrap(), vec![fixes[0].clone()]);
    assert!(source.syntax_fix(usize::MAX).unwrap().is_none());
    let old_version = source.version;
    host.set_text(id, SourceText::from(repaired));
    let current = host.source(id).unwrap();
    assert_ne!(current.version, old_version);
    assert!(current.parse().unwrap().ast().is_some());
    assert!(
        current
            .syntax_fixes(Span::new(0, u32::MAX))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn missing_names_types_and_expressions_are_not_invented_by_repairs() {
    let mut host = AnalysisHost::default();
    for text in [
        "fun () {}",
        "fun f(value: ) {}",
        "fun f() { value. }",
        "fun f() = ;",
    ] {
        let id = host.add_source(SourceText::from(text.to_owned()));
        let source = host.source(id).unwrap();
        assert!(source.parse().unwrap().ast().is_none(), "{text}");
        assert!(
            source
                .syntax_fixes(Span::new(0, text.len() as u32))
                .unwrap()
                .is_empty(),
            "{text}"
        );
    }
}
