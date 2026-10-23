use lsp_server::{ErrorCode, ResponseError};
use lsp_types::Position;
use scoop_analysis::{
    AnalysisSnapshot, Cancellation, ProjectAnalysisError, ProjectInput, ProjectSource,
    SemanticSession, SourceId, SourceInput,
};
use scoop_hir::{self as hir, EditingValue};
use scoop_hir_lower::DeclarationAnalysis;
use scoop_source::Span;

use crate::{convert, initialize::Settings};

mod references;
pub(super) use references::{callable_at, property_at};
mod static_calls;
pub(super) use static_calls::{static_callable, static_named_argument};

pub(super) struct Cursor<'a> {
    pub source: &'a SourceInput,
    pub project: &'a ProjectInput,
    pub file: &'a ProjectSource,
    pub span: Span,
}

pub(super) fn cursor(
    snapshot: &AnalysisSnapshot,
    source_id: SourceId,
    position: Position,
    settings: Settings,
) -> Result<Option<Cursor<'_>>, ResponseError> {
    let Some(mut cursor) = position_cursor(snapshot, source_id, position, settings)? else {
        return Ok(None);
    };
    let parsed = cursor
        .source
        .parse()
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error))?;
    let candidates = match parsed
        .syntax()
        .token_at_offset(scoop_syntax::TextSize::from(cursor.span.start))
    {
        scoop_syntax::TokenAtOffset::None => Vec::new(),
        scoop_syntax::TokenAtOffset::Single(token) => vec![token],
        scoop_syntax::TokenAtOffset::Between(left, right) => vec![right, left],
    };
    let Some(token) = candidates.into_iter().find(|token| {
        matches!(
            token.kind(),
            scoop_syntax::SyntaxKind::Ident
                | scoop_syntax::SyntaxKind::EscapedIdent
                | scoop_syntax::SyntaxKind::KwThis
        )
    }) else {
        return Ok(None);
    };
    cursor.span = Span::new(
        token.text_range().start().into(),
        token.text_range().end().into(),
    );
    Ok(Some(cursor))
}

pub(super) fn position_cursor(
    snapshot: &AnalysisSnapshot,
    source_id: SourceId,
    position: Position,
    settings: Settings,
) -> Result<Option<Cursor<'_>>, ResponseError> {
    let source = snapshot
        .source(source_id)
        .map_err(|_| cancelled())?
        .ok_or_else(|| convert::error(ErrorCode::RequestFailed, "source is unavailable"))?;
    let offset = convert::offset(&source.text, position, settings.encoding)?;
    let span = Span::new(offset, offset);
    let Some(project) = snapshot.workspace.project_for_source(source_id) else {
        return Ok(None);
    };
    let (_, cone) = project
        .source_cone(source_id)
        .expect("selected source belongs to the project");
    let file = cone
        .sources
        .iter()
        .find(|file| file.source == source_id)
        .expect("selected Cone contains the source");
    Ok(Some(Cursor {
        source,
        project,
        file,
        span,
    }))
}

pub(super) fn declarations<'a>(
    semantics: &'a mut SemanticSession,
    snapshot: &AnalysisSnapshot,
    source: SourceId,
    cancellation: &Cancellation,
) -> Result<Option<&'a mut DeclarationAnalysis>, ResponseError> {
    let cancellation = cancellation.clone();
    match semantics.declarations(snapshot, source, move || cancellation.check().is_err()) {
        Ok(analysis) => Ok(analysis),
        Err(
            ProjectAnalysisError::Cancelled
            | ProjectAnalysisError::Declarations(scoop_analysis::DeclarationAnalysisError::Cancelled),
        ) => Err(cancelled()),
        Err(
            ProjectAnalysisError::Artifact(message) | ProjectAnalysisError::Dependency(message),
        ) => Err(convert::error(ErrorCode::RequestFailed, message)),
        Err(_) => Ok(None),
    }
}

pub(super) fn functions(
    analysis: &DeclarationAnalysis,
    cursor: &Cursor<'_>,
) -> Vec<hir::EditingFunction> {
    analysis
        .functions()
        .iter()
        .filter(|function| {
            function.name_origin.source == cursor.file.identity
                && function.span.start <= cursor.span.start
                && cursor.span.end <= function.span.end
        })
        .cloned()
        .collect()
}

pub(super) fn body(
    analysis: &mut DeclarationAnalysis,
    cursor: &Cursor<'_>,
    snapshot: &AnalysisSnapshot,
    cancellation: &Cancellation,
) -> Result<Option<hir::EditingSourceBody>, ResponseError> {
    let revision = snapshot.clone();
    let requested = cancellation.clone();
    analysis
        .source_body_at(&cursor.file.identity, cursor.span, move || {
            requested.check().is_err() || revision.check_cancelled().is_err()
        })
        .map_err(|_| cancelled())
}

pub(super) fn static_variant(
    analysis: &DeclarationAnalysis,
    cursor: &Cursor<'_>,
    snapshot: &AnalysisSnapshot,
    cancellation: &Cancellation,
) -> Result<Option<hir::EditingStaticVariantReference>, ResponseError> {
    match analysis
        .static_variant_reference(&cursor.file.identity, cursor.span, || {
            cancellation.check().is_err() || snapshot.check_cancelled().is_err()
        })
        .map_err(|_| cancelled())?
    {
        hir::EditingValue::Known(reference) => Ok(reference),
        hir::EditingValue::Unavailable(_) => Ok(None),
    }
}

pub(super) struct Local {
    pub reference: Option<hir::EditingLocalReference>,
    pub ty: hir::TypeId,
    pub context: hir::SourceContextId,
}

pub(super) fn local_at(
    analysis: &mut DeclarationAnalysis,
    function: hir::EditingSourceBodyId,
    span: Span,
    snapshot: &AnalysisSnapshot,
    cancellation: &Cancellation,
) -> Result<Option<Local>, ResponseError> {
    let receiver = analysis
        .receivers(function, || {
            cancellation.check().is_err() || snapshot.check_cancelled().is_err()
        })
        .map_err(|_| cancelled())?
        .iter()
        .find(|receiver| receiver.span == span);
    if let Some(receiver) = receiver
        && let EditingValue::Known(ty) = receiver.value_type
    {
        return Ok(Some(Local {
            reference: receiver.local.clone(),
            ty: ty.ty,
            context: ty.context,
        }));
    }
    let snapshot = snapshot.clone();
    let cancellation = cancellation.clone();
    let selector_snapshot = snapshot.clone();
    let selector_cancellation = cancellation.clone();
    if let EditingValue::Known(selectors) = analysis
        .selectors(function, move || {
            selector_cancellation.check().is_err() || selector_snapshot.check_cancelled().is_err()
        })
        .map_err(|_| cancelled())?
        && let Some(selector) = selectors.iter().find(|selector| selector.receiver == span)
    {
        return Ok(Some(Local {
            reference: selector.local.clone(),
            ty: selector.value_type,
            context: selector.context,
        }));
    }
    let EditingValue::Known(facts) = analysis
        .source_body_facts(function, move || {
            cancellation.check().is_err() || snapshot.check_cancelled().is_err()
        })
        .map_err(|_| cancelled())?
    else {
        return Ok(None);
    };
    Ok(facts
        .locals
        .iter()
        .find(|local| local.span == span)
        .map(|local| Local {
            reference: Some(local.reference.clone()),
            ty: local.value_type,
            context: local.context,
        })
        .or_else(|| {
            facts
                .expressions
                .iter()
                .find(|expression| expression.span == span)
                .map(|expression| Local {
                    reference: None,
                    ty: expression.value_type,
                    context: expression.context,
                })
        }))
}

pub(super) fn local_type(
    analysis: &mut DeclarationAnalysis,
    function: hir::EditingSourceBodyId,
    local: &Local,
) -> Result<String, ResponseError> {
    let description = analysis
        .describe_body_type(function, local.context, local.ty)
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error.message))?;
    match description {
        EditingValue::Known(description) => Ok(description.name),
        EditingValue::Unavailable(_) => Err(convert::error(
            ErrorCode::RequestFailed,
            "local type is unavailable",
        )),
    }
}

pub(super) use crate::convert::location;

pub(crate) fn cancelled() -> ResponseError {
    convert::error(ErrorCode::RequestCanceled, "request cancelled")
}
