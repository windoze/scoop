use lsp_server::ResponseError;
use lsp_types::Position;
use scoop_analysis::{AnalysisSnapshot, Cancellation, SemanticSession, SourceId};
use scoop_hir::EditingValue;
use serde_json::Value;

use super::semantic;
use crate::{convert, initialize::Settings};

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
    let Some(analysis) = semantic::declarations(semantics, snapshot, source_id, cancellation)?
    else {
        return Ok(Value::Null);
    };
    for function in semantic::functions(analysis, &cursor) {
        let EditingValue::Known(signature) = analysis.signature(function.function) else {
            continue;
        };
        let mut origin = if function.name_origin.span == cursor.span {
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
        if origin.is_none() {
            origin = semantic::property_at(
                analysis,
                function.function,
                cursor.span,
                snapshot,
                cancellation,
            )?
            .and_then(|reference| reference.definition);
        }
        if origin.is_none() {
            origin = semantic::callable_at(
                analysis,
                function.function,
                cursor.span,
                snapshot,
                cancellation,
            )?
            .and_then(|reference| reference.definition);
        }
        if origin.is_none() {
            let snapshot = snapshot.clone();
            let cancellation = cancellation.clone();
            if let EditingValue::Known(definition) = analysis
                .argument_definition(function.function, cursor.span, move || {
                    cancellation.check().is_err() || snapshot.check_cancelled().is_err()
                })
                .map_err(|_| semantic::cancelled())?
            {
                origin = definition;
            }
        }
        if let Some(origin) = origin {
            return super::json(convert::definition_location(
                snapshot,
                cursor.project,
                &origin,
                settings,
            )?);
        }
    }
    let definitions = analysis
        .import_definitions(&cursor.file.identity, cursor.span, || {
            cancellation.check().is_err() || snapshot.check_cancelled().is_err()
        })
        .map_err(|_| semantic::cancelled())?;
    let mut locations = Vec::new();
    for definition in definitions {
        if let Some(location) =
            convert::definition_location(snapshot, cursor.project, &definition, settings)?
        {
            locations.push(location);
        }
    }
    if locations.len() == 1 {
        return super::json(locations.pop());
    }
    if !locations.is_empty() {
        return super::json(locations);
    }
    Ok(Value::Null)
}
