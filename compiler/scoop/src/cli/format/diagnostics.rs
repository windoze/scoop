use super::super::commands::config;
use scoop_protocol::{
    DiagnosticNoteV1, DiagnosticOriginV1, DiagnosticSeverityV1, HostPathCarrier, ProtocolByteSpan,
    StructuredDiagnosticV1,
};
use scoop_source::{Diagnostic, DiagnosticSeverity, Span};
use std::path::Path;

pub(super) fn convert(
    path: &Path,
    diagnostics: &[Diagnostic],
) -> scoop::BuildResult<Vec<StructuredDiagnosticV1>> {
    let path = HostPathCarrier::from_path(path).map_err(config)?;
    diagnostics
        .iter()
        .map(|diagnostic| {
            let severity = match diagnostic.severity {
                DiagnosticSeverity::Error => DiagnosticSeverityV1::Error,
                DiagnosticSeverity::Warning => DiagnosticSeverityV1::Warning,
            };
            let notes = diagnostic
                .notes
                .iter()
                .map(|note| {
                    DiagnosticNoteV1::new(note.message.clone(), origin(&path, Some(note.span))?)
                        .map_err(config)
                })
                .collect::<scoop::BuildResult<_>>()?;
            StructuredDiagnosticV1::new(
                severity,
                if severity.is_error() {
                    "SCOOPC_PARSE_ERROR"
                } else {
                    "SCOOPC_COMPILER_WARNING"
                }
                .into(),
                diagnostic.message.clone(),
                origin(&path, diagnostic.span)?,
                notes,
            )
            .map_err(config)
        })
        .collect()
}

fn origin(path: &HostPathCarrier, span: Option<Span>) -> scoop::BuildResult<DiagnosticOriginV1> {
    Ok(match span {
        Some(span) => DiagnosticOriginV1::HostPathSpan {
            path: path.clone(),
            span: ProtocolByteSpan::new(span.start.into(), span.end.into()).map_err(config)?,
        },
        None => DiagnosticOriginV1::HostPath { path: path.clone() },
    })
}
