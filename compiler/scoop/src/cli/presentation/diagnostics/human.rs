use super::*;
use scoop_source::{Diagnostic, DiagnosticNote, DiagnosticSeverity, Span};

pub(super) fn render(
    diagnostic: &StructuredDiagnosticV1,
    sources: &[SelectedSource],
    contents: &mut SourceContents,
    color: bool,
) -> String {
    let mut output = match snippet(diagnostic.origin(), sources, contents) {
        Some((path, text, span)) => {
            let mut source_diagnostic = Diagnostic::at(span, diagnostic.message());
            source_diagnostic.severity = match diagnostic.severity() {
                DiagnosticSeverityV1::Error => DiagnosticSeverity::Error,
                DiagnosticSeverityV1::Warning => DiagnosticSeverity::Warning,
            };
            source_diagnostic.render_with_code(&path, text.as_str(), Some(diagnostic.code()), color)
        }
        None => {
            let severity = match diagnostic.severity() {
                DiagnosticSeverityV1::Error => "error",
                DiagnosticSeverityV1::Warning => "warning",
            };
            format!(
                "{}{severity}[{}]: {}",
                location(diagnostic.origin(), sources, contents),
                diagnostic.code(),
                diagnostic.message()
            )
        }
    };
    for note in diagnostic.notes() {
        output.push('\n');
        output.push_str(&match snippet(note.origin(), sources, contents) {
            Some((path, text, span)) => DiagnosticNote::at(0, span, note.message())
                .render_with_color(&path, text.as_str(), color),
            None => format!(
                "{}note: {}",
                location(note.origin(), sources, contents),
                note.message()
            ),
        });
    }
    output
}

fn snippet(
    origin: &DiagnosticOriginV1,
    sources: &[SelectedSource],
    contents: &mut SourceContents,
) -> Option<(String, SourceText, Span)> {
    let (path, text, span) = match origin {
        DiagnosticOriginV1::SourceTextSpan {
            logical_path,
            text,
            span,
            ..
        } => (
            logical_path.to_string(),
            SourceText::new(text.clone()),
            span,
        ),
        DiagnosticOriginV1::SemanticSourceSpan {
            cone,
            logical_path,
            span,
        } => {
            let source = sources.iter().find(|source| {
                source.identity().cone().as_array() == cone.as_array()
                    && source.identity().logical_path() == logical_path
            })?;
            (
                source.display_locator().as_path().display().to_string(),
                read_source(source.physical_path(), contents)?,
                span,
            )
        }
        DiagnosticOriginV1::HostPathSpan { path, span } => {
            let path = path.to_path_buf().ok()?;
            (
                path.display().to_string(),
                read_source(&path, contents)?,
                span,
            )
        }
        DiagnosticOriginV1::None
        | DiagnosticOriginV1::HostPath { .. }
        | DiagnosticOriginV1::ArtifactPath { .. } => return None,
    };
    let span = Span::new(
        u32::try_from(span.start()).ok()?,
        u32::try_from(span.end()).ok()?,
    );
    Some((path, text, span))
}

fn read_source(path: &Path, contents: &mut SourceContents) -> Option<SourceText> {
    contents
        .entry(path.to_owned())
        .or_insert_with(|| std::fs::read_to_string(path).ok().map(SourceText::new))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop_protocol::{DiagnosticNoteV1, HostPathCarrier, ProtocolByteSpan};

    fn at(path: &Path, start: u64, end: u64) -> DiagnosticOriginV1 {
        DiagnosticOriginV1::HostPathSpan {
            path: HostPathCarrier::from_path(path).unwrap(),
            span: ProtocolByteSpan::new(start, end).unwrap(),
        }
    }

    #[test]
    fn human_diagnostics_show_primary_and_related_source_ranges() {
        let path = Path::new("main.scoop");
        let related = Path::new("library.scoop");
        let diagnostic = StructuredDiagnosticV1::new(
            DiagnosticSeverityV1::Error,
            "SCOOPC_HIR_ERROR".into(),
            "value of type Int is not callable".into(),
            at(path, 11, 12),
            vec![DiagnosticNoteV1::new("declared here".into(), at(related, 4, 5)).unwrap()],
        )
        .unwrap();
        let mut contents = SourceContents::from([
            (
                path.to_owned(),
                Some("val x = 1; x(100)\n".to_owned().into()),
            ),
            (related.to_owned(), Some("val x = 1\n".to_owned().into())),
        ]);
        let rendered = render(&diagnostic, &[], &mut contents, false);
        assert!(
            rendered.starts_with("error[SCOOPC_HIR_ERROR]: value of type Int is not callable\n"),
            "{rendered}"
        );
        assert!(rendered.contains("--> main.scoop:1:12"), "{rendered}");
        assert!(
            rendered.contains("1 | val x = 1; x(100)\n  |            ^"),
            "{rendered}"
        );
        assert!(rendered.contains("note: declared here\n"), "{rendered}");
        assert!(rendered.contains("--> library.scoop:1:5"), "{rendered}");
        assert!(!rendered.contains('\u{1b}'), "{rendered:?}");
    }

    #[test]
    fn unavailable_source_keeps_the_diagnostic_and_byte_range() {
        let path = Path::new("missing.scoop");
        let diagnostic = StructuredDiagnosticV1::new(
            DiagnosticSeverityV1::Warning,
            "SCOOPC_COMPILER_WARNING".into(),
            "original warning".into(),
            at(path, 4, 9),
            vec![],
        )
        .unwrap();
        let mut contents = SourceContents::from([(path.to_owned(), None)]);
        assert_eq!(
            render(&diagnostic, &[], &mut contents, false),
            "missing.scoop:bytes 4..9: warning[SCOOPC_COMPILER_WARNING]: original warning"
        );
    }
}
