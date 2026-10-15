use std::fmt;
use std::path::{Path, PathBuf};

use crate::ArtifactCacheRoot;
use crate::cache::COMPILE_CACHE_NAMESPACE;

#[derive(Debug)]
pub(super) struct PreparedStaging {
    root: tempfile::TempDir,
    output_root: PathBuf,
}

impl PreparedStaging {
    pub(super) fn create(cache_root: &ArtifactCacheRoot) -> Result<Self, StagingError> {
        let staging_parent = cache_root
            .as_path()
            .join(COMPILE_CACHE_NAMESPACE)
            .join(".staging");
        create_directory(&staging_parent)?;
        let root = tempfile::Builder::new()
            .prefix("build-")
            .tempdir_in(&staging_parent)
            .map_err(|source| StagingError {
                operation: StagingIoOperation::CreateDirectory,
                path: staging_parent,
                source,
            })?;
        let output_root = root.path().join("output");
        create_directory(&output_root)?;
        Ok(Self { root, output_root })
    }

    pub(super) fn root(&self) -> &Path {
        self.root.path()
    }

    pub(super) fn output_root(&self) -> &Path {
        &self.output_root
    }

    pub(super) fn plan_output(
        &self,
        topological_index: usize,
        cone_directory: &str,
    ) -> Result<PathBuf, StagingError> {
        let root = self
            .output_root
            .join(format!("{topological_index:08}-{cone_directory}"));
        create_directory(&root)?;
        Ok(root.join("candidate.slib"))
    }
}

fn create_directory(path: &Path) -> Result<(), StagingError> {
    std::fs::create_dir_all(path).map_err(|source| StagingError {
        operation: StagingIoOperation::CreateDirectory,
        path: path.to_path_buf(),
        source,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StagingIoOperation {
    CreateDirectory,
    Write,
}

impl fmt::Display for StagingIoOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CreateDirectory => "create directory",
            Self::Write => "write",
        })
    }
}

#[derive(Debug)]
pub struct StagingError {
    operation: StagingIoOperation,
    path: PathBuf,
    source: std::io::Error,
}

impl fmt::Display for StagingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot {} {}: {}",
            self.operation,
            self.path.display(),
            self.source
        )
    }
}

impl std::error::Error for StagingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
