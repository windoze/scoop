use lsp_server::{Message, Notification, RequestId};
use lsp_types::{
    DidChangeTextDocumentParams, DidChangeWatchedFilesParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, LogMessageParams, MessageType,
    PublishDiagnosticsParams,
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
            Ok(Change::Updated(uri)) => self.schedule_diagnostics(&uri),
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
                Ok(())
            }
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
                Ok(Change::Updated(uri))
            }
            "textDocument/didChange" => {
                let params: DidChangeTextDocumentParams = queries::params(notification.params)?;
                let uri = params.text_document.uri.clone();
                self.documents.change(params, self.settings.encoding)?;
                Ok(Change::Updated(uri))
            }
            "textDocument/didSave" => {
                let params: DidSaveTextDocumentParams = queries::params(notification.params)?;
                self.documents.save(&params.text_document.uri, params.text);
                Ok(Change::Updated(params.text_document.uri))
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
                for change in params.changes {
                    self.documents.reload(&change.uri)?;
                }
                Ok(Change::None)
            }
            _ => Ok(Change::None),
        }
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
    Updated(Uri),
    Closed(Uri, Option<String>),
    None,
}
