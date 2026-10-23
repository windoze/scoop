use lsp_server::{ErrorCode, ResponseError};
use lsp_types::{Hover, HoverContents, MarkedString, Position};
use scoop_analysis::{AnalysisSnapshot, Cancellation, SemanticSession, SourceId};
use scoop_hir::{self as hir, EditingValue};
use scoop_hir_lower::DeclarationAnalysis;
use scoop_source::Span;
use serde_json::Value;

use super::semantic;
use crate::{convert, initialize::Settings};

mod arguments;
mod callables;
mod documentation;
mod imported;
mod properties;
mod types;
mod variants;

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
    let descriptions = descriptions(analysis, &cursor, snapshot, cancellation)?;
    if descriptions.is_empty() {
        return super::json(None::<Hover>);
    }
    let mut contents = Vec::new();
    for description in descriptions {
        contents.push(MarkedString::LanguageString(lsp_types::LanguageString {
            language: "scoop".into(),
            value: description.label,
        }));
        if let Some(query) = description.documentation
            && let Some(markdown) = documentation::markdown(
                semantics,
                snapshot,
                source_id,
                &query,
                &cursor,
                settings,
                cancellation,
            )?
            && !markdown.is_empty()
        {
            contents.push(MarkedString::String(markdown));
        }
    }
    super::json(Hover {
        contents: if contents.len() == 1 {
            HoverContents::Scalar(contents.remove(0))
        } else {
            HoverContents::Array(contents)
        },
        range: Some(convert::range(
            &cursor.source.text,
            cursor.span,
            settings.encoding,
        )?),
    })
}

struct Description {
    label: String,
    documentation: Option<documentation::Query>,
}

fn descriptions(
    analysis: &mut DeclarationAnalysis,
    cursor: &semantic::Cursor<'_>,
    snapshot: &AnalysisSnapshot,
    cancellation: &Cancellation,
) -> Result<Vec<Description>, ResponseError> {
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
            return Ok(vec![Description {
                label,
                documentation: Some(documentation::Query {
                    symbol: documentation::callable(hir::SemanticCallable::Current(
                        function.function,
                    )),
                    origin: signature.origin.clone(),
                }),
            }]);
        }
        for (index, parameter) in signature.parameters.iter().enumerate() {
            if parameter.origin.span == target {
                let ty = type_label(analysis, function.function, parameter.value_type)?;
                return Ok(vec![Description {
                    label: format!("{}: {ty}", parameter.name),
                    documentation: Some(documentation::Query {
                        symbol: documentation::parameter(function.function, index),
                        origin: parameter.origin.clone(),
                    }),
                }]);
            }
        }
    }
    if let Some(description) = types::description(analysis, cursor, cancellation)? {
        return Ok(vec![description]);
    }
    if let Some(description) = properties::declaration(analysis, cursor, cancellation)? {
        return Ok(vec![description]);
    }
    if let Some(description) = variants::description(analysis, cursor, snapshot, cancellation)? {
        return Ok(vec![description]);
    }
    if let Some(argument) =
        semantic::static_named_argument(analysis, cursor, snapshot, cancellation)?
    {
        return arguments::description(analysis, argument)
            .map(|description| description.into_iter().collect());
    }
    if let Some(reference) = semantic::static_callable(analysis, cursor, snapshot, cancellation)? {
        return callables::description(analysis, reference, snapshot, cursor)
            .map(|description| description.into_iter().collect());
    }
    if let Some(function) = semantic::body(analysis, cursor, snapshot, cancellation)? {
        let signature = match analysis.source_body_signature(&function) {
            Some(EditingValue::Known(signature)) => Some(signature),
            Some(EditingValue::Unavailable(_)) => return Ok(Vec::new()),
            None => None,
        };
        if let Some(signature) = &signature
            && let Some((index, parameter)) = signature
                .parameters
                .iter()
                .enumerate()
                .find(|(_, parameter)| parameter.origin.span == target)
        {
            let ty = type_label(analysis, signature.function, parameter.value_type)?;
            return Ok(vec![Description {
                label: format!("{}: {ty}", parameter.name),
                documentation: function.callable().map(|callable| documentation::Query {
                    symbol: documentation::applied_parameter(callable.into(), index),
                    origin: parameter.origin.clone(),
                }),
            }]);
        }
        if let Some(description) = arguments::named(analysis, function.id, target, cancellation)? {
            return Ok(vec![description]);
        }
        if let Some(property) =
            semantic::property_at(analysis, function.id, target, snapshot, cancellation)?
        {
            let ty = analysis
                .describe_body_type(function.id, property.context, property.value_type)
                .map_err(|error| convert::error(ErrorCode::RequestFailed, error.message))?;
            if let EditingValue::Known(ty) = ty {
                let kind = if property.mutable { "var" } else { "val" };
                return Ok(vec![Description {
                    label: format!("{kind} {}: {}", property.name, ty.name),
                    documentation: property.definition.map(|origin| documentation::Query {
                        symbol: documentation::property(property.target),
                        origin,
                    }),
                }]);
            }
        }
        if let Some(local) =
            semantic::local_at(analysis, function.id, target, snapshot, cancellation)?
        {
            let ty = semantic::local_type(analysis, function.id, &local)?;
            let label = local
                .reference
                .as_ref()
                .map_or_else(|| ty.clone(), |local| format!("{}: {ty}", local.name));
            return Ok(vec![Description {
                label,
                documentation: local.reference.and_then(|local| {
                    let (callee, index) = analysis.body_parameter(&function, &local.definition)?;
                    Some(documentation::Query {
                        symbol: documentation::applied_parameter(callee, index),
                        origin: local.definition,
                    })
                }),
            }]);
        }
        let mut descriptions = Vec::new();
        for reference in
            semantic::callables_at(analysis, function.id, target, snapshot, cancellation)?
        {
            if let Some(description) =
                callables::description(analysis, reference, snapshot, cursor)?
            {
                descriptions.push(description);
            }
        }
        return Ok(descriptions);
    }
    Ok(Vec::new())
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
