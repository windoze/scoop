use super::*;

#[test]
fn public_default_callables_report_the_restricted_source_occurrence() {
    for (case, expression, highlight, kind) in [
        ("private-function", "secret()", "secret()", "a callable"),
        ("private-reference", "::secret", "::secret", "a callable"),
        (
            "private-setter",
            "this.visible = 3",
            "visible",
            "a property",
        ),
    ] {
        let source =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
                format!("../../tests/fixtures/m23-default-callable-access/errors/{case}.scoop"),
            ))
            .unwrap();
        let Err(diagnostics) =
            lower(&[complete_core_file(), scoop_parser::parse(&source).unwrap()])
        else {
            panic!("{case}: a public default must reject a restricted callable");
        };
        let start =
            (source.rfind(expression).unwrap() + expression.find(highlight).unwrap()) as u32;
        let message = format!(
            "default expression references {kind} outside the callable's complete call domain"
        );
        assert!(
            diagnostics.iter().any(|d| d.file == 1
                && d.span == Some(ast::Span::new(start, start + highlight.len() as u32))
                && d.message == message),
            "{case}: {diagnostics:?}"
        );
    }
}
