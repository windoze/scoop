use lsp_server::{Message, Notification, RequestId};
use lsp_types::{
    DidChangeConfigurationParams, DidChangeTextDocumentParams, DidChangeWatchedFilesParams,
    DidChangeWorkspaceFoldersParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, LogMessageParams, MessageType, PublishDiagnosticsParams,
};
use serde::Deserialize;

use super::*;

#[derive(Deserialize)]
struct CancelParams {
    id: RequestId,
}

impl Server {
    pub(super) fn notification(
        &mut self,
        notification: Notification,
        output: &mut impl Write,
    ) -> io::Result<()> {
        if !self.initialized || self.shutdown {
            return Ok(());
        }
        if notification.method == "initialized" {
            return self.register_input_notifications(output);
        }
        if notification.method == "$/cancelRequest" {
            if let Ok(params) = queries::params::<CancelParams>(notification.params)
                && let Some(pending) = self.pending.remove(&params.id)
            {
                pending.cancellation.cancel();
                self.respond_error(
                    params.id,
                    convert::error(ErrorCode::RequestCanceled, "request cancelled"),
                    output,
                )?;
            }
            return Ok(());
        }
        let changed = self.apply_notification(notification);
        match changed {
            Ok(Change::Updated(uri, workspace)) => {
                if workspace {
                    self.refresh_workspace(output)?;
                }
                self.schedule_diagnostics(&uri)?;
                self.schedule_open_diagnostics()
            }
            Ok(Change::Closed(uri, error)) => {
                self.diagnostics.remove(&uri);
                Message::Notification(Notification::new(
                    "textDocument/publishDiagnostics".to_owned(),
                    PublishDiagnosticsParams {
                        uri,
                        diagnostics: Vec::new(),
                        version: None,
                    },
                ))
                .write(output)?;
                if let Some(error) = error {
                    Self::log(error, output)?;
                }
                self.refresh_workspace(output)
            }
            Ok(Change::Workspace(errors)) => {
                for error in errors {
                    Self::log(error, output)?;
                }
                self.refresh_workspace(output)
            }
            Ok(Change::Resources) => self.schedule_open_diagnostics(),
            Ok(Change::None) => Ok(()),
            Err(error) => Self::log(error.message, output),
        }
    }

    fn apply_notification(
        &mut self,
        notification: Notification,
    ) -> Result<Change, lsp_server::ResponseError> {
        match notification.method.as_str() {
            "textDocument/didOpen" => {
                let params: DidOpenTextDocumentParams = queries::params(notification.params)?;
                let uri = params.text_document.uri.clone();
                self.documents.open(params.text_document)?;
                Ok(Change::Updated(uri, true))
            }
            "textDocument/didChange" => {
                let params: DidChangeTextDocumentParams = queries::params(notification.params)?;
                let uri = params.text_document.uri.clone();
                self.documents.change(params, self.settings.encoding)?;
                let workspace = self.is_manifest(&uri);
                Ok(Change::Updated(uri, workspace))
            }
            "textDocument/didSave" => {
                let params: DidSaveTextDocumentParams = queries::params(notification.params)?;
                self.documents.save(&params.text_document.uri, params.text);
                let workspace = self.is_manifest(&params.text_document.uri);
                Ok(Change::Updated(params.text_document.uri, workspace))
            }
            "textDocument/didClose" => {
                let params: DidCloseTextDocumentParams = queries::params(notification.params)?;
                let error = self
                    .documents
                    .close(&params.text_document.uri)
                    .err()
                    .map(|error| error.message);
                Ok(Change::Closed(params.text_document.uri, error))
            }
            "workspace/didChangeWatchedFiles" => {
                let params: DidChangeWatchedFilesParams = queries::params(notification.params)?;
                let resources_changed = self.resource_watches.changed(&params.changes);
                if resources_changed {
                    self.documents.host.invalidate_compiler_inputs();
                }
                let mut workspace = false;
                let mut errors = Vec::new();
                for change in params.changes {
                    workspace |= super::inputs::is_project_input(&change.uri);
                    if self.documents.get(&change.uri).is_some()
                        && let Err(error) = self.documents.reload(&change.uri)
                    {
                        errors.push(error.message);
                    }
                }
                Ok(if workspace || !errors.is_empty() {
                    Change::Workspace(errors)
                } else if resources_changed {
                    Change::Resources
                } else {
                    Change::None
                })
            }
            "workspace/didChangeConfiguration" => {
                let params: DidChangeConfigurationParams = queries::params(notification.params)?;
                self.workspace_settings.change(params.settings);
                Ok(Change::Workspace(Vec::new()))
            }
            "workspace/didChangeWorkspaceFolders" => {
                let params: DidChangeWorkspaceFoldersParams = queries::params(notification.params)?;
                self.workspace_settings.change_folders(params.event);
                Ok(Change::Workspace(Vec::new()))
            }
            _ => Ok(Change::None),
        }
    }

    fn is_manifest(&self, uri: &Uri) -> bool {
        self.documents
            .get(uri)
            .and_then(|document| document.path)
            .is_some_and(|path| path.file_name().is_some_and(|name| name == "Cone.toml"))
    }

    pub(super) fn log(message: String, output: &mut impl Write) -> io::Result<()> {
        Message::Notification(Notification::new(
            "window/logMessage".to_owned(),
            LogMessageParams {
                typ: MessageType::WARNING,
                message,
            },
        ))
        .write(output)
    }
}

enum Change {
    Updated(Uri, bool),
    Closed(Uri, Option<String>),
    Workspace(Vec<String>),
    Resources,
    None,
}
