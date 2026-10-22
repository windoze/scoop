use lsp_server::ResponseError;
use lsp_types::Position;
use scoop_analysis::{AnalysisSnapshot, Cancellation, SemanticSession, SourceId};
use scoop_hir::EditingValue;
use serde_json::Value;

use super::semantic;
use crate::initialize::Settings;

pub(crate) fn definition(
    snapshot: &AnalysisSnapshot,
    source_id: SourceId,
    position: Position,
    settings: Settings,
    semantics: &mut SemanticSession,
    cancellation: &Cancellation,
) -> Result<Value, ResponseError> {
    let Some(cursor) = semantic::cursor(snapshot, source_id, position, settings)? else {
        return Ok(Value::Null);
    };
    let Some(analysis) = semantic::declarations(semantics, snapshot, source_id)? else {
        return Ok(Value::Null);
    };
    for function in semantic::functions(analysis, &cursor) {
        let EditingValue::Known(signature) = analysis.signature(function.function) else {
            continue;
        };
        let origin = if function.name_origin.span == cursor.span {
            Some(function.name_origin)
        } else if let Some(parameter) = signature
            .parameters
            .iter()
            .find(|parameter| parameter.origin.span == cursor.span)
        {
            Some(parameter.origin.clone())
        } else {
            semantic::local_at(
                analysis,
                function.function,
                cursor.span,
                snapshot,
                cancellation,
            )?
            .and_then(|local| local.reference.map(|reference| reference.definition))
        };
        if let Some(origin) = origin {
            return super::json(semantic::location(
                snapshot,
                cursor.project,
                &origin,
                settings,
            )?);
        }
    }
    Ok(Value::Null)
}
