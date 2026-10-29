//! Resource paths retain their original spelling until filesystem resolution.

use super::paths::{Scope, carrier, io_error, modified};
use scoop_identity::SourceIdentity;
use scoop_protocol::FileInputV1;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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
        let locator = self.locator(source, path)?;
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

    pub fn locator(
        &self,
        source: &SourceIdentity,
        path: &scoop_hir::CompileTimePath,
    ) -> Result<PathBuf, String> {
        let source = self
            .files
            .get(source)
            .ok_or("include call has no physical definition source")?;
        let source = std::fs::canonicalize(source).map_err(|error| io_error(source, error))?;
        let directory = source
            .parent()
            .ok_or("include definition source has no parent directory")?;
        Ok(directory.join(path.as_str()))
    }
}

pub(super) fn current(input: &FileInputV1) -> bool {
    let (Ok(root), Ok(locator)) = (input.root.to_path_buf(), input.locator.to_path_buf()) else {
        return false;
    };
    inspect(&root, &locator).is_ok_and(|observed| observed == *input)
}

fn inspect(root: &Path, locator: &Path) -> Result<FileInputV1, String> {
    let scope = Scope::new(root)?;
    let resolved = scope.resolve(locator)?;
    let metadata = std::fs::metadata(&resolved).map_err(|error| io_error(locator, error))?;
    if !metadata.is_file() {
        return Err(format!(
            "include path `{}` must name a regular file",
            locator.display()
        ));
    }
    Ok(FileInputV1 {
        root: carrier(root)?,
        real_root: carrier(&scope.real_root)?,
        locator: carrier(locator)?,
        resolved: carrier(&resolved)?,
        length: metadata.len(),
        modified: modified(locator, &metadata)?,
    })
}
