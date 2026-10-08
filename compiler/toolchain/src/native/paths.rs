use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use scoop_manifest::{ConeRelativePath, NativeCompileFlag, NativeConfig, NativeIncludeFlag};

use super::{ToolchainError, error};

pub(super) struct InputRoots {
    pub(super) cone: PathBuf,
    pub(super) public: PathBuf,
    pub(super) system: Vec<PathBuf>,
}

impl InputRoots {
    pub(super) fn checked_path(
        &self,
        relative: &ConeRelativePath,
        directory: bool,
    ) -> Result<PathBuf, ToolchainError> {
        let path = self.cone.join(relative.as_path());
        let resolved = path
            .canonicalize()
            .map_err(|e| error(format!("{}: {e}", path.display())))?;
        if !resolved.starts_with(&self.cone) {
            return Err(error(format!(
                "{} escapes the Cone root",
                relative.as_str()
            )));
        }
        let metadata = std::fs::metadata(&resolved).map_err(error)?;
        if (directory && !metadata.is_dir()) || (!directory && !metadata.is_file()) {
            return Err(error(format!(
                "{} must be a {}",
                relative.as_str(),
                if directory {
                    "directory"
                } else {
                    "regular file"
                }
            )));
        }
        Ok(resolved)
    }

    pub(super) fn validate_includes(&self, config: &NativeConfig) -> Result<(), ToolchainError> {
        for include in config.include() {
            self.checked_path(include, true)?;
        }
        for flag in config.c_flags() {
            if let NativeCompileFlag::Include { kind, path } = flag {
                self.checked_path(
                    path,
                    !matches!(
                        kind,
                        NativeIncludeFlag::ForcedHeader | NativeIncludeFlag::Macros
                    ),
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn mappings(&self) -> impl Iterator<Item = (&Path, String)> {
        [
            (&*self.cone, "/scoop-native/cone".to_owned()),
            (&*self.public, "/scoop-native/runtime".to_owned()),
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
        Err(error(format!(
            "{} is outside the Cone, public runtime headers and selected system include directories",
            resolved.display()
        )))
    }

    fn remap(&self, path: &Path) -> Option<String> {
        for (root, prefix) in self.mappings() {
            if let Ok(relative) = path.strip_prefix(root) {
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
) -> Result<Vec<u8>, ToolchainError> {
    let mut result = Vec::with_capacity(bytes.len());
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        if line.starts_with(b"# ") {
            if let Some(start) = line.iter().position(|byte| *byte == b'"') {
                let mut end = start + 1;
                let mut name = Vec::new();
                while end < line.len() && line[end] != b'"' {
                    if line[end] == b'\\' {
                        end += 1;
                    }
                    if end < line.len() {
                        name.push(line[end]);
                        end += 1;
                    }
                }
                let name = std::str::from_utf8(&name).map_err(error)?;
                if let Some(mut logical) = paths
                    .get(Path::new(name))
                    .cloned()
                    .or_else(|| roots.remap(Path::new(name)))
                {
                    if name.ends_with("//") {
                        logical.push('/');
                    }
                    result.extend_from_slice(&line[..start + 1]);
                    result.extend_from_slice(
                        logical
                            .replace('\\', "\\\\")
                            .replace('"', "\\\"")
                            .as_bytes(),
                    );
                    result.extend_from_slice(&line[end..]);
                    continue;
                }
            }
        }
        result.extend_from_slice(line);
    }
    Ok(result)
}

#[cfg(unix)]
pub(super) fn physical_identity(path: &Path) -> Result<(u64, u64), ToolchainError> {
    use std::os::unix::fs::MetadataExt;
    let metadata = std::fs::metadata(path).map_err(error)?;
    Ok((metadata.dev(), metadata.ino()))
}

#[cfg(not(unix))]
pub(super) fn physical_identity(path: &Path) -> Result<PathBuf, ToolchainError> {
    path.canonicalize().map_err(error)
}
