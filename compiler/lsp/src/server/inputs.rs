use std::{
    collections::{BTreeSet, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use lsp_types::FileEvent;
use scoop_toolchain::CompileTimeInputs;

use super::*;

#[derive(Default)]
pub(super) struct ResourceWatches {
    roots: BTreeSet<PathBuf>,
    inputs: Vec<(BTreeSet<PathBuf>, Arc<CompileTimeInputs>)>,
    serial: u64,
    pub requests: HashSet<RequestId>,
}

impl ResourceWatches {
    pub fn changed(&self, changes: &[FileEvent]) -> bool {
        let paths = changes
            .iter()
            .filter_map(|change| event_path(&change.uri))
            .collect::<Vec<_>>();
        self.inputs.iter().any(|(roots, inputs)| {
            paths
                .iter()
                .any(|path| roots.iter().any(|root| path.starts_with(root)))
                && !inputs.inputs_current()
        })
    }

    fn send(
        &mut self,
        method: &str,
        params: serde_json::Value,
        output: &mut impl Write,
    ) -> io::Result<()> {
        let id = RequestId::from(format!("scoop-resources-{}-{method}", self.serial));
        self.requests.insert(id.clone());
        Message::Request(Request::new(id, method.to_owned(), params)).write(output)
    }
}

impl Server {
    pub(super) fn update_resource_watches(
        &mut self,
        inputs: Vec<Arc<CompileTimeInputs>>,
        output: &mut impl Write,
    ) -> io::Result<()> {
        let watches = &mut self.resource_watches;
        watches.inputs = inputs
            .into_iter()
            .filter_map(|input| {
                let roots = input.resource_roots();
                (!roots.is_empty()).then_some((roots, input))
            })
            .collect();
        let roots = watches
            .inputs
            .iter()
            .flat_map(|(roots, _)| roots.iter().cloned())
            .collect();
        if roots == watches.roots {
            return Ok(());
        }
        let old_roots = std::mem::replace(&mut watches.roots, roots);
        if !self.workspace_settings.watch_registration {
            return Ok(());
        }
        let old_id = format!("scoop-resources-{}", watches.serial);
        watches.serial = watches
            .serial
            .checked_add(1)
            .expect("resource watch identifiers exhausted");
        if !watches.roots.is_empty() {
            let relative = self.workspace_settings.relative_watch_patterns;
            let patterns = watches
                .roots
                .iter()
                .flat_map(|root| watch_patterns(root, relative))
                .map(|pattern| serde_json::json!({"globPattern": pattern}))
                .collect::<Vec<_>>();
            watches.send(
                "client/registerCapability",
                serde_json::json!({"registrations": [{
                    "id": format!("scoop-resources-{}", watches.serial),
                    "method": "workspace/didChangeWatchedFiles",
                    "registerOptions": {"watchers": patterns}
                }]}),
                output,
            )?;
        }
        if !old_roots.is_empty() {
            // The misspelling is part of the LSP UnregistrationParams wire format.
            watches.send(
                "client/unregisterCapability",
                serde_json::json!({"unregisterations": [{
                    "id": old_id, "method": "workspace/didChangeWatchedFiles"
                }]}),
                output,
            )?;
        }
        Ok(())
    }
}

pub(super) fn is_project_input(uri: &Uri) -> bool {
    event_path(uri).is_some_and(|path| {
        matches!(
            path.file_name().and_then(|name| name.to_str()),
            Some("Cone.toml" | "Cone.lock")
        ) || matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("scoop" | "slib")
        )
    })
}

fn event_path(uri: &Uri) -> Option<PathBuf> {
    // Keep the event's locator: a deleted or retargeted symlink is the changed input.
    url::Url::parse(uri.as_str()).ok()?.to_file_path().ok()
}

fn literal_glob(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|character| match character {
            '*' | '?' | '[' | ']' | '{' | '}' => format!("[{character}]"),
            character => character.to_string(),
        })
        .collect()
}

fn watch_patterns(root: &Path, relative: bool) -> [serde_json::Value; 2] {
    if relative
        && let (Some(parent), Some(name)) = (root.parent(), root.file_name())
        && let Ok(base) = url::Url::from_directory_path(parent)
    {
        let name = literal_glob(Path::new(name));
        return [name.clone(), format!("{name}/**/*")]
            .map(|pattern| serde_json::json!({"baseUri": base.as_str(), "pattern": pattern}));
    }
    let root = literal_glob(root);
    [root.clone(), format!("{root}/**/*")].map(serde_json::Value::String)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_globs_escape_literal_directory_names() {
        let root = "/cone[1]/{files}/*?";
        let pattern =
            glob::Pattern::new(&format!("{}/**/*", literal_glob(Path::new(root)))).unwrap();
        assert!(pattern.matches(&format!("{root}/nested/file.txt")));
        assert!(!pattern.matches("/cone1/files/any/nested/file.txt"));
    }
}
