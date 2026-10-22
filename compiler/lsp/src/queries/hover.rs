use lsp_server::{ErrorCode, ResponseError};
use lsp_types::{Hover, HoverContents, MarkedString, Position};
use scoop_analysis::{AnalysisSnapshot, ProjectAnalysisError, SemanticSession, SourceId};
use scoop_hir::{self as hir, EditingValue};
use scoop_hir_lower::DeclarationAnalysis;
use scoop_source::Span;
use serde_json::Value;

use crate::{convert, initialize::Settings};

pub(crate) fn hover(
    snapshot: &AnalysisSnapshot,
    source_id: SourceId,
    position: Position,
    settings: Settings,
    semantics: &mut SemanticSession,
) -> Result<Value, ResponseError> {
    let source = snapshot
        .source(source_id)
        .map_err(|_| convert::error(ErrorCode::RequestCanceled, "analysis revision changed"))?
        .ok_or_else(|| convert::error(ErrorCode::RequestFailed, "source is unavailable"))?;
    let offset = convert::offset(&source.text, position, settings.encoding)?;
    let parsed = source
        .parse()
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error))?;
    let syntax = parsed.syntax();
    let candidates = match syntax.token_at_offset(scoop_syntax::TextSize::from(offset)) {
        scoop_syntax::TokenAtOffset::None => Vec::new(),
        scoop_syntax::TokenAtOffset::Single(token) => vec![token],
        scoop_syntax::TokenAtOffset::Between(left, right) => vec![right, left],
    };
    let Some(token) = candidates.into_iter().find(|token| {
        matches!(
            token.kind(),
            scoop_syntax::SyntaxKind::Ident | scoop_syntax::SyntaxKind::EscapedIdent
        )
    }) else {
        return super::json(None::<Hover>);
    };
    let target = Span::new(
        token.text_range().start().into(),
        token.text_range().end().into(),
    );
    let Some(project) = snapshot.workspace.project_for_source(source_id) else {
        return super::json(None::<Hover>);
    };
    let (_, cone) = project
        .source_cone(source_id)
        .expect("selected source belongs to the project");
    let identity = &cone
        .sources
        .iter()
        .find(|file| file.source == source_id)
        .expect("selected Cone contains the source")
        .identity;
    let analysis = match semantics.declarations(snapshot, source_id) {
        Ok(Some(analysis)) => analysis,
        Err(
            ProjectAnalysisError::Cancelled
            | ProjectAnalysisError::Declarations(scoop_analysis::DeclarationAnalysisError::Cancelled),
        ) => {
            return Err(convert::error(
                ErrorCode::RequestCanceled,
                "analysis revision changed",
            ));
        }
        Err(ProjectAnalysisError::Artifact(message)) => {
            return Err(convert::error(ErrorCode::RequestFailed, message));
        }
        Ok(None) | Err(_) => return super::json(None::<Hover>),
    };
    let functions = analysis
        .functions()
        .iter()
        .filter(|function| &function.name_origin.source == identity)
        .cloned()
        .collect::<Vec<_>>();
    for function in functions {
        let EditingValue::Known(signature) = analysis.signature(function.function) else {
            continue;
        };
        if function.name_origin.span == target {
            let label = signature_label(analysis, &signature, source.text.as_str())?;
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
    text: &str,
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
