use std::{
    collections::{BTreeSet, HashMap},
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};

use scoop_identity::ConeIdentity;
use scoop_manifest::LoadedConeManifest;
use scoop_protocol::package::ExpandedConeInputV1;
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
    manifests: HashMap<(String, ConeIdentity, PathBuf), ManifestInput>,
}

struct ManifestInput {
    config: ExpandedConeInputV1,
    manifest: Arc<LoadedConeManifest>,
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

    pub(super) fn expanded(
        &mut self,
        host: &mut AnalysisHost,
        configuration: &str,
        root: ConeIdentity,
        config: ExpandedConeInputV1,
    ) -> Result<Arc<LoadedConeManifest>, LoadError> {
        let path = config
            .manifest_path
            .to_path_buf()
            .map_err(|error| LoadError::new(PathBuf::new(), error))?;
        let key = (configuration.to_owned(), root, path.clone());
        host.set_file_text(path.clone(), SourceText::new(config.manifest_text.clone()));
        if let Some(previous) = self.manifests.get(&key)
            && previous.config == config
        {
            return Ok(previous.manifest.clone());
        }
        let manifest = Arc::new(
            LoadedConeManifest::from_expanded(
                config
                    .root
                    .to_path_buf()
                    .map_err(|error| LoadError::new(&path, error))?,
                path.clone(),
                config.manifest_text.clone(),
                config.manifest.clone(),
                config.metadata.clone(),
            )
            .map_err(|error| LoadError::new(&path, error))?
            .with_input_root(
                config
                    .input_root
                    .to_path_buf()
                    .map_err(|error| LoadError::new(&path, error))?,
            ),
        );
        self.manifests.insert(
            key,
            ManifestInput {
                config,
                manifest: manifest.clone(),
            },
        );
        Ok(manifest)
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
