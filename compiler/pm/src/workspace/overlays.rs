use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use scoop_protocol::package::ManifestOverlaysV1;

use crate::{PmError, PmResult};

#[derive(Clone, Debug, Default)]
pub struct ManifestOverlays(Arc<BTreeMap<PathBuf, String>>);

impl ManifestOverlays {
    pub fn new(input: ManifestOverlaysV1) -> PmResult<Self> {
        input.validate().map_err(PmError::new)?;
        let mut texts = BTreeMap::new();
        for manifest in input.manifests {
            let path = manifest
                .path
                .to_path_buf()
                .map_err(|error| PmError::new(error.to_string()))?;
            let path = existing_or_unsaved(&path).map_err(|error| PmError::io(&path, error))?;
            if texts.insert(path, manifest.text).is_some() {
                return Err(PmError::new("duplicate manifest overlay path"));
            }
        }
        Ok(Self(Arc::new(texts)))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(super) fn text(&self, path: &Path) -> Option<&str> {
        self.0.get(path).map(String::as_str)
    }

    pub(super) fn contains(&self, path: &Path) -> bool {
        !self.is_empty() && existing_or_unsaved(path).is_ok_and(|path| self.0.contains_key(&path))
    }

    pub(super) fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        match std::fs::canonicalize(path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound && self.contains(path) => {
                existing_or_unsaved(path)
            }
            result => result,
        }
    }
}

fn existing_or_unsaved(path: &Path) -> io::Result<PathBuf> {
    match std::fs::canonicalize(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = path
                .parent()
                .ok_or_else(|| io::Error::other("manifest has no parent"))?;
            let name = path
                .file_name()
                .ok_or_else(|| io::Error::other("manifest has no name"))?;
            Ok(std::fs::canonicalize(parent)?.join(name))
        }
        result => result,
    }
}
