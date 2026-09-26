use super::*;

#[test]
fn public_default_values_report_restricted_declaration_occurrences() {
    for (case, expression, kind) in [
        ("private-constructor", "AccessCell(1)", "a constructor"),
        ("private-global", "secret", "a property"),
        ("private-object", "Secret", "a type"),
        ("private-field", "this.secret", "a field"),
    ] {
        let source =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
                format!("../../tests/fixtures/m23-default-value-access/errors/{case}.scoop"),
            ))
            .unwrap();
        let Err(diagnostics) =
            lower(&[complete_core_file(), scoop_parser::parse(&source).unwrap()])
        else {
            panic!("{case}: a public default must reject a restricted value")
        };
        let start = source.rfind(expression).unwrap() as u32;
        let message = format!(
            "default expression references {kind} outside the callable's complete call domain"
        );
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.file == 1
                    && diagnostic.span
                        == Some(ast::Span::new(start, start + expression.len() as u32))
                    && diagnostic.message == message
            }),
            "{case}: {diagnostics:?}"
        );
    }
}
