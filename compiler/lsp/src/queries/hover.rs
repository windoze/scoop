use lsp_server::{ErrorCode, ResponseError};
use lsp_types::{Hover, HoverContents, MarkedString, Position};
use scoop_analysis::{AnalysisSnapshot, Cancellation, SemanticSession, SourceId};
use scoop_hir::{self as hir, EditingValue};
use scoop_hir_lower::DeclarationAnalysis;
use scoop_source::Span;
use serde_json::Value;

use super::semantic;
use crate::{convert, initialize::Settings};

pub(crate) fn hover(
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
    let source = cursor.source;
    let target = cursor.span;
    for function in semantic::functions(analysis, &cursor) {
        let EditingValue::Known(signature) = analysis.signature(function.function) else {
            continue;
        };
        if function.name_origin.span == target {
            let label = signature_label(analysis, &signature)?;
            return response(&source.text, function.name_origin.span, label, settings);
        }
        for parameter in &signature.parameters {
            if parameter.origin.span == target {
                let ty = type_label(analysis, function.function, parameter.value_type)?;
                return response(
                    &source.text,
                    parameter.origin.span,
                    format!("{}: {ty}", parameter.name),
                    settings,
                );
            }
        }
        if let Some(local) =
            semantic::local_at(analysis, function.function, target, snapshot, cancellation)?
        {
            let ty = semantic::local_type(analysis, function.function, &local)?;
            let label = local
                .reference
                .map_or_else(|| ty.clone(), |local| format!("{}: {ty}", local.name));
            return response(&source.text, target, label, settings);
        }
        if let Some(reference) =
            semantic::callable_at(analysis, function.function, target, snapshot, cancellation)?
            && let hir::SemanticCallable::Current(function) = reference.target
            && let EditingValue::Known(signature) = analysis.signature(function)
        {
            let label = signature_label(analysis, &signature)?;
            return response(&source.text, target, label, settings);
        }
    }
    super::json(None::<Hover>)
}

fn response(
    text: &scoop_source::SourceText,
    span: Span,
    label: String,
    settings: Settings,
) -> Result<Value, ResponseError> {
    super::json(Hover {
        contents: HoverContents::Scalar(MarkedString::LanguageString(lsp_types::LanguageString {
            language: "scoop".into(),
            value: label,
        })),
        range: Some(convert::range(text, span, settings.encoding)?),
    })
}

fn type_label(
    analysis: &mut DeclarationAnalysis,
    function: hir::FunctionId,
    ty: hir::TypeId,
) -> Result<String, ResponseError> {
    match analysis
        .describe_type(function, ty)
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error.message))?
    {
        EditingValue::Known(description) => Ok(description.name),
        EditingValue::Unavailable(_) => Err(convert::error(
            ErrorCode::RequestFailed,
            "declaration type is unavailable",
        )),
    }
}

fn signature_label(
    analysis: &mut DeclarationAnalysis,
    signature: &scoop_hir_lower::EditingSignature,
) -> Result<String, ResponseError> {
    let function = signature.function;
    let mut parameters = Vec::with_capacity(signature.parameters.len());
    for parameter in &signature.parameters {
        let (prefix, ty) = parameter
            .vararg_element_type
            .map_or(("", parameter.value_type), |ty| ("vararg ", ty));
        let mut label = format!(
            "{prefix}{}: {}",
            parameter.name,
            type_label(analysis, function, ty)?
        );
        if let Some(default) = parameter.declared_default
            && let Some(text) = analysis.function_source_text(function)
            && let Some(expression) = text.get(default.start as usize..default.end as usize)
        {
            label.push_str(" = ");
            label.push_str(expression);
        }
        parameters.push(label);
    }
    let generics = if signature.type_parameters.is_empty() {
        String::new()
    } else {
        format!(
            "<{}> ",
            signature
                .type_parameters
                .iter()
                .map(|parameter| parameter.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let result = match signature.result {
        EditingValue::Known(ty) => type_label(analysis, function, ty)?,
        EditingValue::Unavailable(_) => "<pending inference>".into(),
    };
    let suspend = if signature.is_suspend { "suspend " } else { "" };
    Ok(format!(
        "{suspend}fun {generics}{}({}): {result}",
        signature.name,
        parameters.join(", ")
    ))
}
