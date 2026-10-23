use lsp_server::{ErrorCode, ResponseError};
use lsp_types::Position;
use scoop_analysis::{
    AnalysisSnapshot, Cancellation, ProjectAnalysisError, SemanticSession, SourceId,
};
use serde_json::Value;

use super::semantic;
use crate::{convert, initialize::Settings};

pub(crate) fn references(
    snapshot: &AnalysisSnapshot,
    source_id: SourceId,
    position: Position,
    include_declaration: bool,
    settings: Settings,
    semantics: &mut SemanticSession,
    cancellation: &Cancellation,
) -> Result<Value, ResponseError> {
    let Some(cursor) = semantic::cursor(snapshot, source_id, position, settings)? else {
        return Ok(Value::Null);
    };
    let requested = cancellation.clone();
    let result = semantics
        .references(snapshot, source_id, cursor.span, move || {
            requested.check().is_err()
        })
        .map_err(|error| match error {
            ProjectAnalysisError::Cancelled => semantic::cancelled(),
            _ => convert::error(
                ErrorCode::RequestFailed,
                "reference analysis is unavailable for this project",
            ),
        })?;
    let Some(result) = result else {
        return Ok(Value::Null);
    };
    let mut locations = Vec::new();
    for reference in result.occurrences {
        cancellation.check().map_err(|_| semantic::cancelled())?;
        snapshot
            .check_cancelled()
            .map_err(|_| semantic::cancelled())?;
        if reference.is_declaration && !include_declaration {
            continue;
        }
        let location = if reference.is_declaration {
            convert::definition_location(snapshot, cursor.project, &reference.origin, settings)?
        } else {
            semantic::location(snapshot, cursor.project, &reference.origin, settings)?
        };
        if let Some(location) = location {
            locations.push(location);
        }
    }
    locations.sort_by(|left, right| {
        (
            left.uri.as_str(),
            left.range.start.line,
            left.range.start.character,
            left.range.end.line,
            left.range.end.character,
        )
            .cmp(&(
                right.uri.as_str(),
                right.range.start.line,
                right.range.start.character,
                right.range.end.line,
                right.range.end.character,
            ))
    });
    locations.dedup();
    super::json(locations)
}
