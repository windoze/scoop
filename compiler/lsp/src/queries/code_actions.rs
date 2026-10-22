use lsp_server::{ErrorCode, ResponseError};
use lsp_types::{
    CodeAction, CodeActionContext, CodeActionKind, DocumentChanges, OneOf,
    OptionalVersionedTextDocumentIdentifier, Range, TextDocumentEdit, TextEdit, Uri, WorkspaceEdit,
};
use scoop_analysis::{Cancellation, SourceInput, SyntaxFix};
use scoop_source::{SourceEdit, Span};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{convert, initialize::Settings};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct ResolveData {
    pub uri: Uri,
    source: u64,
    revision: u64,
    missing: usize,
}

#[derive(Clone)]
pub(crate) struct ActionQuery {
    pub range: Range,
    pub context: CodeActionContext,
}

pub(crate) fn actions(
    uri: &Uri,
    source: &SourceInput,
    version: Option<i32>,
    query: &ActionQuery,
    settings: Settings,
    cancellation: &Cancellation,
) -> Result<Value, ResponseError> {
    if !settings.code_actions {
        return Err(unsupported());
    }
    if query.context.only.as_ref().is_some_and(|kinds| {
        !kinds
            .iter()
            .any(|kind| kind == &CodeActionKind::QUICKFIX || kind == &CodeActionKind::EMPTY)
    }) {
        return super::json(Vec::<CodeAction>::new());
    }
    let range = Span::new(
        convert::offset(&source.text, query.range.start, settings.encoding)?,
        convert::offset(&source.text, query.range.end, settings.encoding)?,
    );
    if range.start > range.end {
        return Err(convert::error(
            ErrorCode::InvalidParams,
            "reversed code action range",
        ));
    }
    let fixes = source
        .syntax_fixes(range)
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error))?;
    let mut actions = Vec::with_capacity(fixes.len());
    for fix in fixes {
        cancellation
            .check()
            .map_err(|_| super::semantic::cancelled())?;
        actions.push(action(uri, source, version, fix, settings)?);
    }
    super::json(actions)
}

pub(crate) fn resolve(
    data: ResolveData,
    source: &SourceInput,
    version: Option<i32>,
    settings: Settings,
) -> Result<Value, ResponseError> {
    if !settings.code_action_resolve {
        return Err(unsupported());
    }
    if source.version.source.0 != data.source || source.version.revision.0 != data.revision {
        return Err(convert::error(
            ErrorCode::ContentModified,
            "code action source changed",
        ));
    }
    let fix = source
        .syntax_fix(data.missing)
        .map_err(|error| convert::error(ErrorCode::RequestFailed, error))?
        .ok_or_else(|| convert::error(ErrorCode::InvalidParams, "syntax repair is unavailable"))?;
    super::json(action(&data.uri, source, version, fix, settings)?)
}

fn action(
    uri: &Uri,
    source: &SourceInput,
    version: Option<i32>,
    fix: SyntaxFix,
    settings: Settings,
) -> Result<CodeAction, ResponseError> {
    let data = settings.code_action_resolve.then(|| ResolveData {
        uri: uri.clone(),
        source: fix.edit.source.source.0,
        revision: fix.edit.source.revision.0,
        missing: fix.missing,
    });
    Ok(CodeAction {
        title: format!("Insert `{}`", fix.edit.replacement),
        kind: Some(CodeActionKind::QUICKFIX),
        edit: Some(workspace_edit(uri, source, version, fix.edit, settings)?),
        data: data.map(super::json).transpose()?,
        ..Default::default()
    })
}

fn workspace_edit(
    uri: &Uri,
    source: &SourceInput,
    version: Option<i32>,
    edit: SourceEdit,
    settings: Settings,
) -> Result<WorkspaceEdit, ResponseError> {
    Ok(WorkspaceEdit {
        document_changes: Some(DocumentChanges::Edits(vec![TextDocumentEdit {
            text_document: OptionalVersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version,
            },
            edits: vec![OneOf::Left(TextEdit {
                range: convert::range(&source.text, edit.span, settings.encoding)?,
                new_text: edit.replacement,
            })],
        }])),
        ..Default::default()
    })
}

fn unsupported() -> ResponseError {
    convert::error(
        ErrorCode::MethodNotFound,
        "client does not support versioned code actions",
    )
}
