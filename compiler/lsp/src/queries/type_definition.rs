use lsp_server::{ErrorCode, ResponseError};
use lsp_types::Position;
use scoop_analysis::{AnalysisSnapshot, Cancellation, SemanticSession, SourceId};
use scoop_hir::{self as hir, EditingValue as V};
use scoop_hir_lower::DeclarationAnalysis;
use serde_json::Value;

use super::semantic;
use crate::{convert, initialize::Settings};

pub(crate) fn type_definition(
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
    let Some(origin) = origin(analysis, &cursor, snapshot, cancellation)? else {
        return Ok(Value::Null);
    };
    super::json(convert::definition_location(
        snapshot,
        cursor.project,
        &origin,
        settings,
    )?)
}

fn origin(
    analysis: &mut DeclarationAnalysis,
    cursor: &semantic::Cursor<'_>,
    snapshot: &AnalysisSnapshot,
    cancellation: &Cancellation,
) -> Result<Option<hir::SemanticSourceLocation>, ResponseError> {
    let functions = semantic::functions(analysis, cursor);
    for function in &functions {
        let V::Known(signature) = analysis.signature(function.function) else {
            continue;
        };
        if function.name_origin.span == cursor.span {
            return match signature.result {
                V::Known(ty) => value_origin(analysis, ty),
                V::Unavailable(_) => Ok(None),
            };
        }
        if let Some(parameter) = signature
            .parameters
            .iter()
            .find(|parameter| parameter.origin.span == cursor.span)
        {
            return value_origin(analysis, parameter.value_type);
        }
    }
    if let V::Known(Some(ty)) = analysis
        .source_type_application(
            &cursor.file.identity,
            cursor.span,
            check(snapshot, cancellation),
        )
        .map_err(|_| semantic::cancelled())?
    {
        return value_origin(analysis, ty);
    }
    if let V::Known(Some(reference)) = analysis
        .type_reference(
            &cursor.file.identity,
            cursor.span,
            check(snapshot, cancellation),
        )
        .map_err(|_| semantic::cancelled())?
        && matches!(
            reference.target,
            hir::EditingTypeIdentity::Nominal(_) | hir::EditingTypeIdentity::Parameter(_)
        )
    {
        return Ok(reference.definition);
    }
    if let V::Known(Some(property)) = analysis
        .property_declaration(
            &cursor.file.identity,
            cursor.span,
            check(snapshot, cancellation),
        )
        .map_err(|_| semantic::cancelled())?
    {
        return value_origin(analysis, property.value_type);
    }
    if let Some(reference) = semantic::static_variant(analysis, cursor, snapshot, cancellation)? {
        return value_origin(analysis, reference.value_type);
    }
    if let Some(argument) =
        semantic::static_named_argument(analysis, cursor, snapshot, cancellation)?
    {
        return value_origin(analysis, argument.parameter.value_type);
    }
    if let V::Known(calls) = analysis
        .static_calls(&cursor.file.identity, check(snapshot, cancellation))
        .map_err(|_| semantic::cancelled())?
        && let Some(call) = calls
            .iter()
            .find(|call| call.callee_span == Some(cursor.span))
    {
        return value_origin(analysis, call.result);
    }
    if let Some(function) = semantic::body(analysis, cursor, snapshot, cancellation)? {
        if let Some(V::Known(signature)) = analysis.source_body_signature(&function)
            && let Some(parameter) = signature
                .parameters
                .iter()
                .find(|parameter| parameter.origin.span == cursor.span)
        {
            return value_origin(analysis, parameter.value_type);
        }
        if let V::Known(calls) = analysis
            .calls(function.id, check(snapshot, cancellation))
            .map_err(|_| semantic::cancelled())?
            && let Some(ty) = calls
                .iter()
                .find(|call| call.callee_span == Some(cursor.span))
                .map(|call| call.result)
        {
            return value_origin(analysis, ty);
        }
        if let Some(property) =
            semantic::property_at(analysis, function.id, cursor.span, snapshot, cancellation)?
        {
            return value_origin(analysis, property.value_type);
        }
        if let Some(local) =
            semantic::local_at(analysis, function.id, cursor.span, snapshot, cancellation)?
        {
            return value_origin(analysis, local.ty);
        }
    }
    Ok(None)
}

fn value_origin(
    analysis: &mut DeclarationAnalysis,
    ty: hir::TypeId,
) -> Result<Option<hir::SemanticSourceLocation>, ResponseError> {
    analysis
        .type_definition(ty)
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error.message))
}

fn check(snapshot: &AnalysisSnapshot, cancellation: &Cancellation) -> impl Fn() -> bool + 'static {
    let snapshot = snapshot.clone();
    let cancellation = cancellation.clone();
    move || cancellation.check().is_err() || snapshot.check_cancelled().is_err()
}
