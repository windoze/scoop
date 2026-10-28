use lsp_server::{Notification, ResponseError};
use lsp_types::PublishDiagnosticsParams;

use super::*;
use crate::worker::Completed;

impl Server {
    pub(super) fn completed(
        &mut self,
        completion: Completion,
        output: &mut impl Write,
    ) -> io::Result<()> {
        if completion.revision == self.documents.host.revision() {
            self.update_resource_watches(completion.inputs, output)?;
        }
        match completion.value {
            Completed::Request {
                id,
                version,
                result,
            } => {
                let Some(pending) = self.pending.get(&id) else {
                    return Ok(());
                };
                if completion.revision != self.documents.host.revision()
                    || pending.query.document_uri().is_some_and(|uri| {
                        self.documents
                            .get(uri)
                            .and_then(|document| document.version)
                            != version
                    })
                {
                    return self.schedule_request(&id, output);
                }
                let result = if matches!(&pending.query, Query::Document(query) if matches!(query.kind, crate::queries::QueryKind::Rename { .. }))
                {
                    result.and_then(|value| self.rename_versions(value))
                } else {
                    result
                };
                self.pending.remove(&id);
                Message::Response(Response {
                    id,
                    response_result: result,
                })
                .write(output)
            }
            Completed::Diagnostics {
                uri,
                version,
                result,
            } => {
                if self.diagnostics.get(&uri) != Some(&(completion.revision, version)) {
                    return Ok(());
                }
                self.diagnostics.remove(&uri);
                if completion.revision != self.documents.host.revision() {
                    return self.schedule_diagnostics(&uri);
                }
                if self
                    .documents
                    .get(&uri)
                    .and_then(|document| document.version)
                    != Some(version)
                {
                    return Ok(());
                }
                match result {
                    Ok(diagnostics) => Message::Notification(Notification::new(
                        "textDocument/publishDiagnostics".to_owned(),
                        PublishDiagnosticsParams {
                            uri,
                            diagnostics,
                            version: Some(version),
                        },
                    ))
                    .write(output),
                    Err(error) => {
                        Message::Notification(Notification::new(
                            "textDocument/publishDiagnostics".to_owned(),
                            PublishDiagnosticsParams {
                                uri,
                                diagnostics: Vec::new(),
                                version: Some(version),
                            },
                        ))
                        .write(output)?;
                        Self::log(error.message, output)
                    }
                }
            }
        }
    }

    fn rename_versions(
        &self,
        value: serde_json::Value,
    ) -> Result<serde_json::Value, ResponseError> {
        if value.is_null() {
            return Ok(value);
        }
        let mut edit: lsp_types::WorkspaceEdit = serde_json::from_value(value)
            .map_err(|error| convert::error(ErrorCode::InternalError, error))?;
        if let Some(lsp_types::DocumentChanges::Edits(documents)) = &mut edit.document_changes {
            for document in documents {
                if let Some(path) = crate::documents::file_path(&document.text_document.uri)?
                    && let Some(uri) = self.documents.uri_for_path(&path)
                {
                    document.text_document.uri = uri.clone();
                }
                document.text_document.version = self
                    .documents
                    .get(&document.text_document.uri)
                    .and_then(|document| document.version);
            }
        }
        crate::queries::json(edit)
    }
}
