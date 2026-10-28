//! Shared Cone bounds for direct files and every directory entry.

use scoop_protocol::HostPathCarrier;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

pub(super) struct Scope {
    pub root: PathBuf,
    pub real_root: PathBuf,
}

impl Scope {
    pub fn new(root: &Path) -> Result<Self, String> {
        let real_root = std::fs::canonicalize(root).map_err(|error| io_error(root, error))?;
        Ok(Self {
            root: root.to_owned(),
            real_root,
        })
    }

    pub fn resolve(&self, locator: &Path) -> Result<PathBuf, String> {
        let normalized = normalize(locator);
        if !normalized.starts_with(normalize(&self.root))
            && !normalized.starts_with(&self.real_root)
        {
            return Err(format!(
                "include path `{}` is outside the defining Cone directory `{}`",
                locator.display(),
                self.root.display()
            ));
        }
        // Open the original spelling: link/.. follows the filesystem, not lexical cleanup.
        let resolved = std::fs::canonicalize(locator).map_err(|error| io_error(locator, error))?;
        if !resolved.starts_with(&self.real_root) {
            return Err(format!(
                "include symlink target `{}` is outside the defining Cone directory `{}`",
                resolved.display(),
                self.real_root.display()
            ));
        }
        Ok(resolved)
    }
}

pub(super) fn modified(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(bool, u64, u32), String> {
    let modified = metadata.modified().map_err(|error| io_error(path, error))?;
    let (negative, elapsed) = match modified.duration_since(UNIX_EPOCH) {
        Ok(value) => (false, value),
        Err(error) => (true, error.duration()),
    };
    Ok((negative, elapsed.as_secs(), elapsed.subsec_nanos()))
}

pub(super) fn carrier(value: &Path) -> Result<HostPathCarrier, String> {
    HostPathCarrier::from_path(value).map_err(|error| error.to_string())
}

fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                result.pop();
            }
            component => result.push(component.as_os_str()),
        }
    }
    result
}

pub(super) fn io_error(path: &Path, error: std::io::Error) -> String {
    format!("cannot read include input `{}`: {error}", path.display())
}
