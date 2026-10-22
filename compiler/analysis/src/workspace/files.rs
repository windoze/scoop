use std::{
    collections::{BTreeSet, HashMap},
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use scoop_manifest::{LoadedConeManifest, ManifestRootLocator};
use scoop_source::{SourceId, SourceText};

use super::LoadError;
use crate::AnalysisHost;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FileStamp {
    length: u64,
    modified: SystemTime,
}

impl FileStamp {
    pub(super) fn read(path: &Path) -> Result<Self, LoadError> {
        let metadata = path
            .metadata()
            .map_err(|error| LoadError::new(path, error))?;
        Ok(Self {
            length: metadata.len(),
            modified: metadata
                .modified()
                .map_err(|error| LoadError::new(path, error))?,
        })
    }
}

#[derive(Default)]
pub(super) struct FileInputs {
    disk: HashMap<PathBuf, (FileStamp, SourceText)>,
    manifests: HashMap<PathBuf, ManifestInput>,
}

struct ManifestInput {
    text: SourceText,
    result: Result<Arc<LoadedConeManifest>, LoadError>,
}

impl FileInputs {
    pub(super) fn read(
        &mut self,
        host: &mut AnalysisHost,
        overlays: &BTreeSet<PathBuf>,
        path: &Path,
    ) -> Result<(SourceId, SourceText), LoadError> {
        if overlays.contains(path)
            && let Some(id) = host.file_source(path)
            && let Some(source) = host.source(id)
        {
            return Ok((id, source.text.clone()));
        }
        let stamp = FileStamp::read(path)?;
        let text = match self.disk.get(path) {
            Some((previous, text)) if *previous == stamp => text.clone(),
            _ => {
                let text = SourceText::new(
                    std::fs::read_to_string(path).map_err(|error| LoadError::new(path, error))?,
                );
                self.disk.insert(path.to_owned(), (stamp, text.clone()));
                text
            }
        };
        let id = host.set_file_text(path.to_owned(), text.clone());
        Ok((id, text))
    }

    pub(super) fn manifest(
        &mut self,
        host: &mut AnalysisHost,
        overlays: &BTreeSet<PathBuf>,
        root: &Path,
    ) -> Result<Arc<LoadedConeManifest>, LoadError> {
        let locator = if root.file_name().is_some_and(|name| name == "Cone.toml") {
            ManifestRootLocator::exact_manifest_file(root)
        } else {
            ManifestRootLocator::cone_directory(root)
        };
        let located = locator
            .resolve()
            .map_err(|error| LoadError::new(error.path(), &error))?;
        let path = located.manifest_path();
        let (_, text) = self.read(host, overlays, path)?;
        if let Some(previous) = self.manifests.get(path)
            && previous.text == text
        {
            return previous.result.clone();
        }
        let parsed = scoop_toolchain::parse_cone_manifest(text.as_str()).map_err(|error| {
            let mut diagnostic = LoadError::new(path, &error);
            if let Some(span) = error.span() {
                diagnostic = diagnostic.with_span(span.range());
            }
            diagnostic
        });
        let path = path.to_owned();
        let result = parsed.and_then(|parsed| {
            located
                .with_parsed(text.as_str().to_owned(), parsed)
                .map(Arc::new)
                .map_err(|error| LoadError::new(error.path(), &error))
        });
        self.manifests.insert(
            path,
            ManifestInput {
                text,
                result: result.clone(),
            },
        );
        result
    }
}

/// Resolve ordinary file aliases while still accepting an unsaved file path.
pub fn normalize_path(path: &Path) -> std::io::Result<PathBuf> {
    match std::fs::canonicalize(path) {
        Ok(path) => return Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let path = std::path::absolute(path)?;
    if let Some(parent) = path.parent()
        && let Ok(parent) = std::fs::canonicalize(parent)
        && let Some(name) = path.file_name()
    {
        return Ok(parent.join(name));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    Ok(normalized)
}
