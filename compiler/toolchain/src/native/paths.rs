use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use super::{ToolchainError, error};

pub(super) struct InputRoots {
    pub(super) cone: PathBuf,
    pub(super) public: PathBuf,
    pub(super) target: PathBuf,
    pub(super) system: Vec<PathBuf>,
}

impl InputRoots {
    pub(super) fn check_source(&self, relative: &Path) -> Result<(), ToolchainError> {
        let path = self.cone.join(relative);
        let metadata = path
            .metadata()
            .map_err(|e| error(format!("{}: {e}", path.display())))?;
        if !metadata.is_file() {
            return Err(error(format!(
                "{} must be a regular file",
                relative.display()
            )));
        }
        Ok(())
    }

    pub(super) fn mappings(&self) -> impl Iterator<Item = (&Path, String)> {
        [
            (&*self.cone, "/scoop-native/cone".to_owned()),
            (&*self.public, "/scoop-native/runtime".to_owned()),
            (&*self.target, "/scoop-native/target".to_owned()),
        ]
        .into_iter()
        .chain(
            self.system
                .iter()
                .enumerate()
                .map(|(index, root)| (root.as_path(), format!("/scoop-native/system/{index}"))),
        )
    }

    pub(super) fn logical(&self, path: &Path) -> Result<String, ToolchainError> {
        let resolved = path.canonicalize().map_err(error)?;
        if let Some(logical) = self.remap(&resolved) {
            return Ok(logical);
        }
        Ok(resolved.to_string_lossy().into_owned())
    }

    fn remap(&self, path: &Path) -> Option<String> {
        for (root, prefix) in self.mappings() {
            if let Ok(relative) = dunce::simplified(path).strip_prefix(dunce::simplified(root)) {
                return Some(format!("{prefix}/{}", relative.to_str()?));
            }
        }
        None
    }
}

pub(super) fn normalize_line_markers(
    bytes: &[u8],
    paths: &BTreeMap<PathBuf, String>,
    roots: &InputRoots,
    search: &[PathBuf],
    observed: &mut BTreeSet<PathBuf>,
) -> Result<Vec<u8>, ToolchainError> {
    super::includes::transform(bytes, &roots.cone, search, observed, |path| {
        paths
            .get(path)
            .cloned()
            .or_else(|| roots.remap(path))
            .or_else(|| {
                path.canonicalize()
                    .ok()
                    .and_then(|path| paths.get(&path).cloned().or_else(|| roots.remap(&path)))
            })
    })
}
