use lsp_server::{ErrorCode, ResponseError};
use lsp_types::{Hover, HoverContents, MarkedString, Position};
use scoop_analysis::{AnalysisSnapshot, Cancellation, SemanticSession, SourceId};
use scoop_hir::{self as hir, EditingValue};
use scoop_hir_lower::DeclarationAnalysis;
use scoop_source::Span;
use serde_json::Value;

use super::semantic;
use crate::{convert, initialize::Settings};

mod documentation;
mod imported;
mod properties;
mod types;

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
    let Some(description) = description(analysis, &cursor, snapshot, cancellation)? else {
        return super::json(None::<Hover>);
    };
    let documentation = match description.documentation {
        Some(query) => documentation::markdown(
            semantics,
            snapshot,
            source_id,
            &query,
            &cursor,
            settings,
            cancellation,
        )?,
        None => None,
    };
    response(
        &cursor.source.text,
        cursor.span,
        description.label,
        documentation,
        settings,
    )
}

struct Description {
    label: String,
    documentation: Option<documentation::Query>,
}

fn description(
    analysis: &mut DeclarationAnalysis,
    cursor: &semantic::Cursor<'_>,
    snapshot: &AnalysisSnapshot,
    cancellation: &Cancellation,
) -> Result<Option<Description>, ResponseError> {
    let target = cursor.span;
    let functions = semantic::functions(analysis, cursor)
        .into_iter()
        .filter_map(|function| match analysis.signature(function.function) {
            EditingValue::Known(signature) => Some((function, signature)),
            EditingValue::Unavailable(_) => None,
        })
        .collect::<Vec<_>>();
    for (function, signature) in &functions {
        if function.name_origin.span == target {
            let label = signature_label(analysis, signature)?;
            return Ok(Some(Description {
                label,
                documentation: Some(documentation::Query {
                    symbol: documentation::callable(hir::SemanticCallable::Current(
                        function.function,
                    )),
                    origin: signature.origin.clone(),
                }),
            }));
        }
        for (index, parameter) in signature.parameters.iter().enumerate() {
            if parameter.origin.span == target {
                let ty = type_label(analysis, function.function, parameter.value_type)?;
                return Ok(Some(Description {
                    label: format!("{}: {ty}", parameter.name),
                    documentation: Some(documentation::Query {
                        symbol: documentation::parameter(function.function, index),
                        origin: parameter.origin.clone(),
                    }),
                }));
            }
        }
    }
    if let Some(description) = types::description(analysis, cursor, cancellation)? {
        return Ok(Some(description));
    }
    if let Some(description) = properties::declaration(analysis, cursor, cancellation)? {
        return Ok(Some(description));
    }
    for (function, signature) in functions {
        if let Some(property) =
            semantic::property_at(analysis, function.function, target, snapshot, cancellation)?
        {
            let ty = analysis
                .describe_body_type(function.function, property.context, property.value_type)
                .map_err(|error| convert::error(ErrorCode::RequestFailed, error.message))?;
            if let EditingValue::Known(ty) = ty {
                let kind = if property.mutable { "var" } else { "val" };
                return Ok(Some(Description {
                    label: format!("{kind} {}: {}", property.name, ty.name),
                    documentation: property.definition.map(|origin| documentation::Query {
                        symbol: documentation::property(property.target),
                        origin,
                    }),
                }));
            }
        }
        if let Some(local) =
            semantic::local_at(analysis, function.function, target, snapshot, cancellation)?
        {
            let ty = semantic::local_type(analysis, function.function, &local)?;
            let label = local
                .reference
                .as_ref()
                .map_or_else(|| ty.clone(), |local| format!("{}: {ty}", local.name));
            return Ok(Some(Description {
                label,
                documentation: local.reference.and_then(|local| {
                    let index = signature
                        .parameters
                        .iter()
                        .position(|parameter| parameter.origin == local.definition)?;
                    Some(documentation::Query {
                        symbol: documentation::parameter(function.function, index),
                        origin: local.definition,
                    })
                }),
            }));
        }
        if let Some(reference) =
            semantic::callable_at(analysis, function.function, target, snapshot, cancellation)?
        {
            let mut origin = reference.definition;
            let label = match reference.target {
                hir::SemanticCallable::Current(function) => match analysis.signature(function) {
                    EditingValue::Known(signature) => {
                        let label = signature_label(analysis, &signature)?;
                        origin = Some(signature.origin);
                        Some(label)
                    }
                    EditingValue::Unavailable(_) => None,
                },
                hir::SemanticCallable::Imported(callable) => {
                    imported::label(analysis, callable, snapshot, cursor.project)?
                }
                hir::SemanticCallable::CurrentGetter(_)
                | hir::SemanticCallable::CurrentSetter(_) => None,
            };
            if let Some(label) = label {
                return Ok(Some(Description {
                    label,
                    documentation: origin.map(|origin| documentation::Query {
                        symbol: documentation::callable(reference.target),
                        origin,
                    }),
                }));
            }
        }
    }
    Ok(None)
}

fn response(
    text: &scoop_source::SourceText,
    span: Span,
    label: String,
    documentation: Option<String>,
    settings: Settings,
) -> Result<Value, ResponseError> {
    let signature = MarkedString::LanguageString(lsp_types::LanguageString {
        language: "scoop".into(),
        value: label,
    });
    super::json(Hover {
        contents: match documentation.filter(|text| !text.is_empty()) {
            Some(documentation) => {
                HoverContents::Array(vec![signature, MarkedString::String(documentation)])
            }
            None => HoverContents::Scalar(signature),
        },
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
