use super::*;

#[test]
fn default_expansion_cycles_have_definition_site_diagnostics() {
    for (source, offending, parameter, callable) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-default-preparation/errors/self-cycle.scoop"
            )),
            "cycle()",
            "value",
            "cycle",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-default-preparation/errors/mutual-cycle.scoop"
            )),
            "first()",
            "value",
            "first",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-default-preparation/errors/constructor-cycle.scoop"
            )),
            "Cycle()",
            "value",
            "Cycle",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-default-preparation/errors/variant-cycle.scoop"
            )),
            "helper()",
            "value",
            "helper",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-default-preparation/errors/vararg-cycle.scoop"
            )),
            "cycle()",
            "values",
            "cycle",
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-default-preparation/errors/local-cycle.scoop"
            )),
            "cycle()",
            "value",
            "cycle",
        ),
    ] {
        let diagnostics = lower_source(source).unwrap_err();
        let diagnostic = diagnostics
            .iter()
            .find(|d| d.message.starts_with("cyclic default expansion"))
            .unwrap_or_else(|| panic!("{diagnostics:?}"));
        let start = source.find(offending).unwrap() as u32;
        assert_eq!(diagnostic.file, 1);
        assert_eq!(
            diagnostic.span,
            Some(ast::Span::new(start, start + offending.len() as u32))
        );
        assert!(
            diagnostic
                .message
                .contains(&format!("parameter `{parameter}`"))
        );
        assert!(
            diagnostic.message.ends_with(&format!("in `{callable}`")),
            "{diagnostic:?}"
        );
        assert_eq!(
            diagnostics
                .iter()
                .filter(|d| d.message.starts_with("cyclic default expansion"))
                .count(),
            1
        );
    }
}

#[test]
fn a_failed_forward_default_does_not_publish_a_partial_call() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-default-preparation/errors/invalid-forward.scoop"
    ));
    let diagnostics = lower_source(source).unwrap_err();
    let diagnostic = diagnostics
        .iter()
        .find(|d| {
            d.message
                .contains("default value of parameter `value` in `invalid`")
        })
        .unwrap_or_else(|| panic!("{diagnostics:?}"));
    let start = source.find("\"wrong\"").unwrap() as u32;
    assert_eq!(diagnostic.span, Some(ast::Span::new(start, start + 7)));
    assert!(
        diagnostic
            .message
            .ends_with("must be of type Int, found String")
    );
}
