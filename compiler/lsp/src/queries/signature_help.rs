use lsp_server::{ErrorCode, ResponseError};
use lsp_types::{
    ParameterInformation, ParameterLabel, Position, SignatureHelp, SignatureInformation,
};
use scoop_analysis::{AnalysisSnapshot, Cancellation, SemanticSession, SourceId};
use scoop_hir::{self as hir, EditingValue};
use scoop_hir_lower::DeclarationAnalysis;
use serde_json::Value;

use super::semantic;
use crate::{convert, initialize::Settings};

mod incomplete;
mod position;

pub(crate) fn signature_help(
    snapshot: &AnalysisSnapshot,
    source: SourceId,
    position: Position,
    settings: Settings,
    semantics: &mut SemanticSession,
    cancellation: &Cancellation,
) -> Result<Value, ResponseError> {
    let Some(cursor) = semantic::position_cursor(snapshot, source, position, settings)? else {
        return Ok(Value::Null);
    };
    let syntax = cursor
        .source
        .parse()
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error))?
        .syntax();
    let positions = position::argument_lists(&syntax, cursor.span.start);
    let Some(analysis) = semantic::declarations(semantics, snapshot, source, cancellation)? else {
        return Ok(Value::Null);
    };
    let mut calls = match analysis
        .static_calls(&cursor.file.identity, || {
            cancellation.check().is_err() || snapshot.check_cancelled().is_err()
        })
        .map_err(|_| semantic::cancelled())?
    {
        EditingValue::Known(calls) => calls,
        EditingValue::Unavailable(_) => Vec::new(),
    };
    if let Some(function) = semantic::body(analysis, &cursor, snapshot, cancellation)? {
        let revision = snapshot.clone();
        let cancelled = cancellation.clone();
        let EditingValue::Known(body_calls) = analysis
            .calls(function.id, move || {
                cancelled.check().is_err() || revision.check_cancelled().is_err()
            })
            .map_err(|_| semantic::cancelled())?
        else {
            return incomplete::signature_help(analysis, &cursor, snapshot, cancellation);
        };
        calls.extend_from_slice(body_calls);
    }
    let selected = calls
        .into_iter()
        .filter_map(|call| {
            let active = position::active_parameter(&call, cursor.span.start, &positions)?;
            Some((call, active))
        })
        .min_by_key(|(call, _)| call.span.end - call.span.start);
    let Some((call, active_parameter)) = selected else {
        return incomplete::signature_help(analysis, &cursor, snapshot, cancellation);
    };
    let mut labels = Vec::with_capacity(call.parameters.len());
    for parameter in &call.parameters {
        let ty = type_label(
            analysis,
            &call.context,
            parameter
                .vararg_element_type
                .unwrap_or(parameter.value_type),
        )?;
        let prefix = if parameter.vararg_element_type.is_some() {
            "vararg "
        } else {
            ""
        };
        labels.push(match &parameter.name {
            Some(name) => format!(
                "{prefix}{}: {ty}",
                scoop_analysis::identifier_spelling(name)
            ),
            None => ty,
        });
    }
    let name = call
        .callee_span
        .and_then(|span| {
            syntax
                .token_at_offset(scoop_syntax::TextSize::from(span.start))
                .right_biased()
                .filter(|token| u32::from(token.text_range().end()) == span.end)
                .filter(|token| token.kind() == scoop_syntax::SyntaxKind::KwThis)
                .map(|token| std::borrow::Cow::Owned(token.text().to_owned()))
        })
        .or_else(|| {
            call.name
                .as_deref()
                .map(scoop_analysis::identifier_spelling)
        })
        .unwrap_or_default();
    let result = type_label(analysis, &call.context, call.result)?;
    super::json(SignatureHelp {
        signatures: vec![SignatureInformation {
            label: format!("{name}({}): {result}", labels.join(", ")),
            documentation: None,
            parameters: Some(
                labels
                    .into_iter()
                    .map(|label| ParameterInformation {
                        label: ParameterLabel::Simple(label),
                        documentation: None,
                    })
                    .collect(),
            ),
            active_parameter: active_parameter.map(|index| index as u32),
        }],
        active_signature: Some(0),
        active_parameter: active_parameter.map(|index| index as u32),
    })
}

fn type_label(
    analysis: &mut DeclarationAnalysis,
    context: &hir::EditingCallContext,
    ty: hir::TypeId,
) -> Result<String, ResponseError> {
    match analysis
        .describe_call_type(context, ty)
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error.message))?
    {
        EditingValue::Known(description) => Ok(description.name),
        EditingValue::Unavailable(_) => Err(convert::error(
            ErrorCode::RequestFailed,
            "call type is unavailable",
        )),
    }
}
