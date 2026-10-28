//! Resource paths retain their original spelling until filesystem resolution.

use scoop_identity::SourceIdentity;
use scoop_protocol::{FileInputV1, HostPathCarrier};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

pub(super) struct Sources {
    pub root: PathBuf,
    pub files: BTreeMap<SourceIdentity, PathBuf>,
}

#[derive(Clone)]
pub(super) struct ReadFile {
    pub bytes: Arc<[u8]>,
    pub input: FileInputV1,
}

impl Sources {
    pub fn read(
        &self,
        source: &SourceIdentity,
        path: &scoop_hir::CompileTimePath,
    ) -> Result<ReadFile, String> {
        let source = self
            .files
            .get(source)
            .ok_or("include call has no physical definition source")?;
        let source = std::fs::canonicalize(source).map_err(|error| io_error(source, error))?;
        let directory = source
            .parent()
            .ok_or("include definition source has no parent directory")?;
        let locator = directory.join(path.as_str());
        let input = inspect(&self.root, &locator)?;
        let resolved = input
            .resolved
            .to_path_buf()
            .map_err(|error| error.to_string())?;
        let bytes = std::fs::read(&resolved)
            .map_err(|error| io_error(&locator, error))?
            .into();
        Ok(ReadFile { bytes, input })
    }
}

pub(super) fn current(input: &FileInputV1) -> bool {
    let (Ok(root), Ok(locator)) = (input.root.to_path_buf(), input.locator.to_path_buf()) else {
        return false;
    };
    inspect(&root, &locator).is_ok_and(|observed| observed == *input)
}

fn inspect(root: &Path, locator: &Path) -> Result<FileInputV1, String> {
    let real_root = std::fs::canonicalize(root).map_err(|error| io_error(root, error))?;
    let normalized = normalize(locator);
    if !normalized.starts_with(normalize(root)) && !normalized.starts_with(&real_root) {
        return Err(format!(
            "include path `{}` is outside the defining Cone directory `{}`",
            locator.display(),
            root.display()
        ));
    }
    // Do not open `normalized`: `link/..` must follow the original filesystem path.
    let resolved = std::fs::canonicalize(locator).map_err(|error| io_error(locator, error))?;
    if !resolved.starts_with(&real_root) {
        return Err(format!(
            "include symlink target `{}` is outside the defining Cone directory `{}`",
            resolved.display(),
            real_root.display()
        ));
    }
    let metadata = std::fs::metadata(&resolved).map_err(|error| io_error(locator, error))?;
    if !metadata.is_file() {
        return Err(format!(
            "include path `{}` must name a regular file",
            locator.display()
        ));
    }
    let modified = metadata
        .modified()
        .map_err(|error| io_error(locator, error))?;
    let (negative, elapsed) = match modified.duration_since(UNIX_EPOCH) {
        Ok(value) => (false, value),
        Err(error) => (true, error.duration()),
    };
    let path = |value: &Path| HostPathCarrier::from_path(value).map_err(|error| error.to_string());
    Ok(FileInputV1 {
        root: path(root)?,
        real_root: path(&real_root)?,
        locator: path(locator)?,
        resolved: path(&resolved)?,
        length: metadata.len(),
        modified: (negative, elapsed.as_secs(), elapsed.subsec_nanos()),
    })
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

fn io_error(path: &Path, error: std::io::Error) -> String {
    format!("cannot read include input `{}`: {error}", path.display())
}
