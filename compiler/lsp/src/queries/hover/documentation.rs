use super::*;

mod targets;
pub(super) use targets::{applied_parameter, callable, parameter, property};

pub(super) struct Query {
    pub symbol: hir::DocumentationSymbol,
    pub origin: hir::SemanticSourceLocation,
}

pub(super) fn markdown(
    semantics: &mut SemanticSession,
    snapshot: &AnalysisSnapshot,
    source: SourceId,
    query: &Query,
    cursor: &semantic::Cursor<'_>,
    settings: Settings,
    cancellation: &Cancellation,
) -> Result<Option<String>, ResponseError> {
    let cancelled = cancellation.clone();
    let view = semantics
        .documentation(snapshot, source, &query.origin, move || {
            cancelled.check().is_err()
        })
        .map_err(|error| match error {
            scoop_analysis::ProjectAnalysisError::Cancelled
            | scoop_analysis::ProjectAnalysisError::Declarations(
                scoop_analysis::DeclarationAnalysisError::Cancelled,
            ) => semantic::cancelled(),
            _ => convert::error(
                ErrorCode::RequestFailed,
                "documentation inputs are unavailable",
            ),
        })?;
    let Some(view) = view else { return Ok(None) };
    let Some(content) = view.get(query.symbol) else {
        return Ok(None);
    };
    let mut failure = None;
    let text = scoop_doc::hover_markdown(content, |symbol| {
        let origin = view.source_locations.get(&symbol)?;
        if cancellation.check().is_err() || snapshot.check_cancelled().is_err() {
            failure = Some(semantic::cancelled());
            return None;
        }
        match convert::definition_location(snapshot, cursor.project, origin, settings) {
            Ok(Some(location)) => Some(format!(
                "{}#L{}",
                location.uri.as_str(),
                location.range.start.line + 1
            )),
            Ok(None) => None,
            Err(error) => {
                failure = Some(error);
                None
            }
        }
    });
    if let Some(error) = failure {
        return Err(error);
    }
    if cancellation.check().is_err() || snapshot.check_cancelled().is_err() {
        return Err(semantic::cancelled());
    }
    Ok(Some(text))
}
