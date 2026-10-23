use std::collections::BTreeMap;

use lsp_server::{ErrorCode, ResponseError};
use lsp_types::{
    DocumentChanges, OneOf, OptionalVersionedTextDocumentIdentifier, Position, TextDocumentEdit,
    TextEdit, WorkspaceEdit,
};
use scoop_analysis::{AnalysisSnapshot, Cancellation, RenameError, SemanticSession, SourceId};
use serde_json::Value;

use super::semantic;
use crate::{convert, initialize::Settings};

pub(crate) fn prepare_rename(
    snapshot: &AnalysisSnapshot,
    source: SourceId,
    position: Position,
    settings: Settings,
    semantics: &mut SemanticSession,
    cancellation: &Cancellation,
) -> Result<Value, ResponseError> {
    supported(settings)?;
    let Some(cursor) = semantic::cursor(snapshot, source, position, settings)? else {
        return Ok(Value::Null);
    };
    let cancellation = cancellation.clone();
    let Some(prepared) = semantics
        .prepare_rename(snapshot, source, cursor.span, move || {
            cancellation.check().is_err()
        })
        .map_err(error)?
    else {
        return Ok(Value::Null);
    };
    Ok(serde_json::json!({
        "range": convert::range(&cursor.source.text, prepared.span, settings.encoding)?,
        "placeholder": prepared.placeholder,
    }))
}

pub(crate) fn rename(
    snapshot: &AnalysisSnapshot,
    source: SourceId,
    position: Position,
    new_name: &str,
    settings: Settings,
    semantics: &mut SemanticSession,
    cancellation: &Cancellation,
) -> Result<Value, ResponseError> {
    supported(settings)?;
    let Some(cursor) = semantic::cursor(snapshot, source, position, settings)? else {
        return Ok(Value::Null);
    };
    let cancellation = cancellation.clone();
    let Some(edits) = semantics
        .rename(snapshot, source, cursor.span, new_name, move || {
            cancellation.check().is_err()
        })
        .map_err(error)?
    else {
        return Ok(Value::Null);
    };
    let mut documents = BTreeMap::<String, TextDocumentEdit>::new();
    for edit in edits {
        let file = cursor
            .project
            .cones
            .values()
            .filter_map(|cone| cone.source())
            .flat_map(|cone| &cone.sources)
            .find(|file| file.source == edit.source.source)
            .ok_or_else(|| {
                convert::error(ErrorCode::RequestFailed, "rename source is unavailable")
            })?;
        let origin = scoop_hir::SemanticSourceLocation {
            source: file.identity.clone(),
            span: edit.span,
        };
        let location =
            convert::location(snapshot, cursor.project, &origin, settings)?.ok_or_else(|| {
                convert::error(
                    ErrorCode::RequestFailed,
                    "rename source location is unavailable",
                )
            })?;
        let document = documents
            .entry(location.uri.as_str().to_owned())
            .or_insert_with(|| TextDocumentEdit {
                text_document: OptionalVersionedTextDocumentIdentifier {
                    uri: location.uri.clone(),
                    version: None,
                },
                edits: Vec::new(),
            });
        document.edits.push(OneOf::Left(TextEdit {
            range: location.range,
            new_text: edit.replacement,
        }));
    }
    super::json(WorkspaceEdit {
        document_changes: Some(DocumentChanges::Edits(documents.into_values().collect())),
        ..Default::default()
    })
}

fn supported(settings: Settings) -> Result<(), ResponseError> {
    if settings.renames {
        Ok(())
    } else {
        Err(convert::error(
            ErrorCode::MethodNotFound,
            "client does not support versioned rename edits",
        ))
    }
}

fn error(error: RenameError) -> ResponseError {
    let code = match &error {
        RenameError::Cancelled
        | RenameError::Edit(scoop_analysis::SnapshotEditError::Cancelled) => {
            ErrorCode::RequestCanceled
        }
        RenameError::InvalidName => ErrorCode::InvalidParams,
        RenameError::Edit(scoop_analysis::SnapshotEditError::SourceChanged(_)) => {
            ErrorCode::ContentModified
        }
        _ => ErrorCode::RequestFailed,
    };
    convert::error(code, error)
}
