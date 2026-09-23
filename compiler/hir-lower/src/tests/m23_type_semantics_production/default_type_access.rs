use super::*;

#[test]
fn public_default_types_report_the_restricted_declaration_occurrence() {
    for (case, expression) in [
        ("private-type", "HiddenType()"),
        ("restricted-owner", "Hidden.Exposed()"),
    ] {
        let source =
            std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
                format!("../../tests/fixtures/m23-default-type-access/errors/{case}.scoop"),
            ))
            .unwrap();
        let Err(diagnostics) =
            lower(&[complete_core_file(), scoop_parser::parse(&source).unwrap()])
        else {
            panic!("{case}: a public default must reject a restricted type")
        };
        let start = source.rfind(expression).unwrap() as u32;
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.file == 1
                && diagnostic.span == Some(ast::Span::new(start, start + expression.len() as u32))
                && diagnostic.message == "default expression references a type outside the callable's complete call domain"
        }), "{case}: {diagnostics:?}");
    }
}
