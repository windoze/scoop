use super::*;
use scoop_ast::DiagnosticNote;
use scoop_identity::{ConeCoordinate, NormalizedSourcePath};

fn source(cone: u8, path: &str) -> SourceIdentity {
    SourceIdentity::new(
        ConeCoordinate::new("test", &format!("cone{cone}"), "1.0.0")
            .unwrap()
            .identity()
            .unwrap(),
        NormalizedSourcePath::new(path).unwrap(),
    )
    .unwrap()
}

#[test]
fn primary_and_notes_keep_independent_canonical_sources() {
    let current = source(19, "src/main.scoop");
    let dependency = source(23, "api/value.scoop");
    let mut diagnostic = Diagnostic::at_file(7, Span::new(12, 19), "ambiguous value");
    diagnostic.source = Some(Box::new(current.clone()));
    let mut note = DiagnosticNote::at(7, Span::new(40, 45), "candidate declared here");
    note.source = Some(Box::new(dependency.clone()));
    diagnostic.notes.push(note);
    diagnostic.resolve_sources(&[source(99, "unrelated.scoop")]);

    let actual = convert(&diagnostic, "SCOOPC_HIR_ERROR", |_| None).unwrap();
    assert_eq!(
        actual.origin(),
        &source_origin(Some(&current), Span::new(12, 19)).unwrap()
    );
    assert_eq!(
        actual.notes()[0].origin(),
        &source_origin(Some(&dependency), Span::new(40, 45)).unwrap()
    );
    assert_eq!(actual.message(), "ambiguous value");
}

#[test]
fn warning_retains_severity_span_and_multifile_note() {
    let sources = [source(19, "src/a.scoop"), source(19, "src/b.scoop")];
    let diagnostic = Diagnostic::warning_at_file(1, Span::new(7, 12), "unreachable branch")
        .with_note(DiagnosticNote::at(0, Span::new(1, 4), "covered here"));
    let actual = convert(&diagnostic, "SCOOPC_HIR_ERROR", |index| sources.get(index)).unwrap();
    assert_eq!(actual.severity(), DiagnosticSeverityV1::Warning);
    assert_eq!(actual.code(), "SCOOPC_COMPILER_WARNING");
    assert_eq!(
        actual.origin(),
        &source_origin(Some(&sources[1]), Span::new(7, 12)).unwrap()
    );
    assert_eq!(
        actual.notes()[0].origin(),
        &source_origin(Some(&sources[0]), Span::new(1, 4)).unwrap()
    );
}

#[test]
fn unresolved_source_is_an_internal_error_and_not_a_tool_diagnostic() {
    let diagnostic = Diagnostic::at_file(42, Span::new(3, 8), "bad expression");
    let error = convert(&diagnostic, "SCOOPC_HIR_ERROR", |_| None).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("diagnostic source index is unresolved")
    );
    let tool = Diagnostic::without_span(DiagnosticSeverity::Error, 0, "compiler unavailable");
    assert_eq!(
        convert(&tool, "SCOOPC_BUILD_FAILED", |_| None)
            .unwrap()
            .origin(),
        &DiagnosticOriginV1::None
    );
}

#[test]
fn unavailable_dependency_text_renders_bytes_without_invented_line_numbers() {
    let identity = source(23, "api/value.scoop");
    let mut note = DiagnosticNote::at(1, Span::new(40, 45), "candidate declared here");
    note.source = Some(Box::new(identity.clone()));
    let set = CurrentConeDiagnosticSet {
        diagnostics: vec![
            Diagnostic::warning_at(Span::new(0, 1), "ambiguous value").with_note(note),
        ],
        sources: vec![
            super::super::CurrentConeDiagnosticSource {
                identity: source(19, "src/main.scoop"),
                display_locator: "src/main.scoop".to_owned(),
                source_text: Some("value".to_owned()),
            },
            super::super::CurrentConeDiagnosticSource {
                display_locator: format!("{}/{}", identity.cone(), identity.logical_path()),
                identity,
                source_text: None,
            },
        ],
    };
    let text = set.render_human();
    assert!(text.starts_with("src/main.scoop:1:1: warning: ambiguous value\n"));
    assert!(text.ends_with("/api/value.scoop:bytes 40..45: note: candidate declared here"));
}
