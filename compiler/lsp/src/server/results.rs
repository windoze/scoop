use lsp_server::Notification;
use lsp_types::PublishDiagnosticsParams;

use super::*;
use crate::worker::Completed;

impl Server {
    pub(super) fn completed(
        &mut self,
        completion: Completion,
        output: &mut impl Write,
    ) -> io::Result<()> {
        match completion.value {
            Completed::Request { id, result } => {
                if !self.pending.contains_key(&id) {
                    return Ok(());
                }
                if completion.revision != self.documents.host.revision() {
                    return self.schedule_request(&id, output);
                }
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
}
