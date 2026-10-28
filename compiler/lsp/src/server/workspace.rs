use std::collections::BTreeMap;

use lsp_server::Notification;
use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, PublishDiagnosticsParams};

use super::*;

impl Server {
    pub(super) fn refresh_workspace(&mut self, output: &mut impl Write) -> io::Result<()> {
        let (configurations, mut errors, active) =
            self.workspace_settings.configurations(&self.documents);
        let overlays = self.documents.open_paths();
        let mut inputs = self.workspace_loader.load(
            &mut self.documents.host,
            &configurations,
            &overlays,
            active,
        );
        inputs.diagnostics.append(&mut errors);
        self.documents.host.set_workspace(inputs);
        self.update_resource_watches(Vec::new(), output)?;
        self.publish_project_diagnostics(output)?;
        self.schedule_open_diagnostics()
    }

    pub(super) fn schedule_open_diagnostics(&mut self) -> io::Result<()> {
        for uri in self.documents.open_uris() {
            self.schedule_diagnostics(&uri)?;
        }
        Ok(())
    }

    // Uri only caches parsed components; its spelling, equality and ordering do not change.
    #[allow(clippy::mutable_key_type)]
    fn publish_project_diagnostics(&mut self, output: &mut impl Write) -> io::Result<()> {
        let mut grouped = BTreeMap::<Uri, Vec<Diagnostic>>::new();
        for diagnostic in &self.documents.host.workspace().diagnostics {
            let Ok(url) = url::Url::from_file_path(&diagnostic.path) else {
                continue;
            };
            let Ok(uri) = url.as_str().parse::<Uri>() else {
                continue;
            };
            let uri = self
                .documents
                .uri_for_path(&diagnostic.path)
                .cloned()
                .unwrap_or(uri);
            let range = diagnostic
                .span
                .and_then(|span| {
                    let source = self.documents.host.file_source(&diagnostic.path)?;
                    let text = &self.documents.host.source(source)?.text;
                    convert::range(text, span, self.settings.encoding).ok()
                })
                .unwrap_or_default();
            grouped.entry(uri).or_default().push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String("SCOOP_PROJECT".to_owned())),
                source: Some("scoop".to_owned()),
                message: format!("{}: {}", diagnostic.configuration, diagnostic.message),
                ..Diagnostic::default()
            });
        }
        for uri in self.project_diagnostics.keys() {
            grouped.entry(uri.clone()).or_default();
        }
        let mut current = HashMap::new();
        for (uri, diagnostics) in grouped {
            let version = self
                .documents
                .get(&uri)
                .and_then(|document| document.version);
            if !self
                .project_diagnostics
                .get(&uri)
                .is_some_and(|(old_version, old_diagnostics)| {
                    *old_version == version && old_diagnostics == &diagnostics
                })
            {
                Message::Notification(Notification::new(
                    "textDocument/publishDiagnostics".to_owned(),
                    PublishDiagnosticsParams {
                        uri: uri.clone(),
                        diagnostics: diagnostics.clone(),
                        version,
                    },
                ))
                .write(output)?;
            }
            if !diagnostics.is_empty() {
                current.insert(uri, (version, diagnostics));
            }
        }
        self.project_diagnostics = current;
        Ok(())
    }

    pub(super) fn register_input_notifications(&self, output: &mut impl Write) -> io::Result<()> {
        if !self.workspace_settings.watch_registration {
            return Ok(());
        }
        Message::Request(Request::new(
            RequestId::from("scoop-watch-inputs".to_owned()),
            "client/registerCapability".to_owned(),
            serde_json::json!({"registrations": [{
                "id": "scoop-inputs", "method": "workspace/didChangeWatchedFiles",
                "registerOptions": {"watchers": [
                    {"globPattern": "**/Cone.toml"}, {"globPattern": "**/Cone.lock"}, {"globPattern": "**/*.scoop"}, {"globPattern": "**/*.slib"}
                ]}
            }]}),
        )).write(output)
    }
}
