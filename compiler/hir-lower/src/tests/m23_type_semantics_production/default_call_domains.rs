use super::*;

#[test]
fn invalid_default_call_domains_report_the_source_occurrence() {
    for (case, expression, highlight, message) in [
        (
            "protected-after-public",
            "protected override fun choose",
            "choose",
            "visibility of `choose` does not cover inherited slot `PublicMiddle.choose`",
        ),
        (
            "new-override-default",
            "public override fun choose",
            "choose",
            "override function `choose` cannot declare a new default expression",
        ),
        (
            "widened-default-access",
            "seed()",
            "seed()",
            "default expression references a callable outside the callable's complete call domain",
        ),
        (
            "restricted-provider-type",
            "value: Int = 1",
            "1",
            "default expression references a type outside the callable's complete call domain",
        ),
    ] {
        let source =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
                format!("../../tests/fixtures/m23-default-call-domains/errors/{case}.scoop"),
            ))
            .unwrap();
        let Err(diagnostics) =
            lower(&[complete_core_file(), scoop_parser::parse(&source).unwrap()])
        else {
            panic!("{case}: invalid default call domains must fail");
        };
        let start =
            (source.rfind(expression).unwrap() + expression.find(highlight).unwrap()) as u32;
        assert!(
            diagnostics.iter().any(|d| d.file == 1
                && d.span == Some(ast::Span::new(start, start + highlight.len() as u32))
                && d.message == message),
            "{case}: {diagnostics:?}"
        );
    }
}
