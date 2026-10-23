use super::*;
use scoop_generator_protocol::{GeneratorDiagnostic, GeneratorSeverity, SourceLocation};
use scoop_identity::{NormalizedSourcePath, SourceIdentity};

pub(super) fn resolve(
    queries: &dyn hir::SemanticQueries,
    reported: GeneratorDiagnostic,
    fallback: &hir::SemanticSourceLocation,
) -> Result<Diagnostic, Diagnostic> {
    let invalid =
        |message: String| diagnostic(fallback, format!("invalid generator diagnostic: {message}"));
    let location = reported
        .location
        .map(|location| resolve_location(queries, location))
        .transpose()
        .map_err(invalid)?
        .unwrap_or_else(|| fallback.clone());
    let mut result = diagnostic(&location, reported.message);
    result.severity = match reported.severity {
        GeneratorSeverity::Error => scoop_ast::DiagnosticSeverity::Error,
        GeneratorSeverity::Warning => scoop_ast::DiagnosticSeverity::Warning,
    };
    for note in reported.notes {
        let location = resolve_location(queries, note.location).map_err(invalid)?;
        result.notes.push(related(&location, note.message));
    }
    if location.source != fallback.source || location.span != fallback.span {
        result
            .notes
            .push(related(fallback, "implementation requested here"));
    }
    Ok(result)
}

fn resolve_location(
    queries: &dyn hir::SemanticQueries,
    location: SourceLocation,
) -> Result<hir::SemanticSourceLocation, String> {
    let cone = queries
        .resolve_provider(location.source.cone.into())
        .ok_or_else(|| "unknown source provider".to_owned())?;
    let path =
        NormalizedSourcePath::new(&location.source.path).map_err(|error| error.to_string())?;
    let source = SourceIdentity::new(cone, path);
    if location.span.start_byte > location.span.end_byte {
        return Err("source range ends before it starts".into());
    }
    Ok(hir::SemanticSourceLocation {
        source,
        span: Span::new(location.span.start_byte, location.span.end_byte),
    })
}
