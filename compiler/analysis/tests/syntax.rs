use scoop_analysis::{AnalysisHost, FoldKind, folding_ranges};
use scoop_source::SourceText;

#[test]
fn block_imports_fold_only_consecutive_imports_in_the_same_scope() {
    let source = r#"import api.first
import api.second
fun example() {
    import api.outer
    import api.other
    val value = 1;
    {
        import api.inner
        import api.nested
    }
    import api.last
    val callback = {
        import api.callback
        import api.result
        result()
    }
}
"#;
    let mut host = AnalysisHost::default();
    let id = host.add_source(SourceText::from(source.to_owned()));
    let parsed = host.source(id).unwrap().parse().unwrap();
    assert!(parsed.ast().is_some());
    let imports = folding_ranges(&parsed.syntax())
        .into_iter()
        .filter(|fold| fold.kind == FoldKind::Imports)
        .map(|fold| source[fold.span.start as usize..fold.span.end as usize].trim())
        .collect::<Vec<_>>();
    assert_eq!(
        imports,
        [
            "import api.first\nimport api.second",
            "import api.outer\n    import api.other",
            "import api.inner\n        import api.nested",
            "import api.last",
            "import api.callback\n        import api.result",
        ]
    );
}
