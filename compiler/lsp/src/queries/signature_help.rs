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
    let mut selected = None;
    for function in semantic::functions(analysis, &cursor) {
        let revision = snapshot.clone();
        let cancelled = cancellation.clone();
        let EditingValue::Known(calls) = analysis
            .calls(function.function, move || {
                cancelled.check().is_err() || revision.check_cancelled().is_err()
            })
            .map_err(|_| semantic::cancelled())?
        else {
            continue;
        };
        for call in calls {
            let Some(active) = position::active_parameter(call, cursor.span.start, &positions)
            else {
                continue;
            };
            let width = call.span.end - call.span.start;
            if selected
                .as_ref()
                .is_none_or(|(_, previous, _, _)| width < *previous)
            {
                selected = Some((function.function, width, call.clone(), active));
            }
        }
    }
    let Some((function, _, call, active_parameter)) = selected else {
        return Ok(Value::Null);
    };
    let mut labels = Vec::with_capacity(call.parameters.len());
    for parameter in &call.parameters {
        let ty = type_label(
            analysis,
            function,
            call.context,
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
        .name
        .as_deref()
        .map(scoop_analysis::identifier_spelling)
        .unwrap_or_default();
    let result = type_label(analysis, function, call.context, call.result)?;
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
    function: hir::FunctionId,
    context: hir::SourceContextId,
    ty: hir::TypeId,
) -> Result<String, ResponseError> {
    match analysis
        .describe_body_type(function, context, ty)
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error.message))?
    {
        EditingValue::Known(description) => Ok(description.name),
        EditingValue::Unavailable(_) => Err(convert::error(
            ErrorCode::RequestFailed,
            "call type is unavailable",
        )),
    }
}
