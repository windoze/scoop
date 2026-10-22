use lsp_server::{ErrorCode, ResponseError};
use lsp_types::{Diagnostic, Location, Uri};
use scoop_analysis::{
    AnalysisSnapshot, DeclarationAnalysisError, ProjectAnalysisError, SemanticSession, SourceId,
    SourceInput,
};

use crate::{convert, initialize::Settings};

pub(crate) fn collect(
    snapshot: &AnalysisSnapshot,
    source_id: SourceId,
    source: &SourceInput,
    uri: &Uri,
    settings: Settings,
    semantics: &mut SemanticSession,
) -> Result<Vec<Diagnostic>, ResponseError> {
    let parsed = source
        .parse()
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error))?;
    let mut diagnostics = Vec::new();
    for diagnostic in parsed.diagnostics() {
        if let Some(diagnostic) =
            convert::diagnostic(&source.text, diagnostic, settings.encoding, |note| {
                Ok(Some(Location {
                    uri: uri.clone(),
                    range: convert::range(&source.text, note.span, settings.encoding)?,
                }))
            })?
        {
            diagnostics.push(diagnostic);
        }
    }
    let semantic = match semantics.diagnostics(snapshot, source_id) {
        Ok(diagnostics) => diagnostics,
        Err(
            ProjectAnalysisError::ConfigurationUnavailable
            | ProjectAnalysisError::SourceDependencyUnavailable(_),
        ) => Vec::new(),
        Err(
            ProjectAnalysisError::Cancelled
            | ProjectAnalysisError::Declarations(DeclarationAnalysisError::Cancelled),
        ) => {
            return Err(convert::error(
                ErrorCode::RequestCanceled,
                "analysis revision changed",
            ));
        }
        Err(ProjectAnalysisError::Artifact(message)) => {
            return Err(convert::error(ErrorCode::RequestFailed, message));
        }
        Err(error) => {
            return Err(convert::error(
                ErrorCode::RequestFailed,
                format!("{error:?}"),
            ));
        }
    };
    for diagnostic in &semantic {
        if let Some(diagnostic) =
            convert::diagnostic(&source.text, diagnostic, settings.encoding, |note| {
                let (Some(project), Some(identity)) = (
                    snapshot.workspace.project_for_source(source_id),
                    note.source.as_deref(),
                ) else {
                    return Ok(None);
                };
                convert::location(
                    snapshot,
                    project,
                    &scoop_hir::SemanticSourceLocation {
                        source: identity.clone(),
                        span: note.span,
                    },
                    settings,
                )
            })?
        {
            diagnostics.push(diagnostic);
        }
    }
    diagnostics.sort_by(|a, b| {
        a.range
            .start
            .cmp(&b.range.start)
            .then(a.range.end.cmp(&b.range.end))
            .then(a.message.cmp(&b.message))
    });
    diagnostics.dedup();
    Ok(diagnostics)
}
