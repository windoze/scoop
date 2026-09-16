use std::fmt;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use scoop_wire::Digest256;

use crate::cache::COMPILE_CACHE_NAMESPACE;
use crate::{ArtifactCacheRoot, ImmutableInputSnapshot};

const STAGING_DIRECTORY: &str = ".staging";

#[derive(Debug)]
pub(super) struct PreparedStaging {
    root: tempfile::TempDir,
    input_root: PathBuf,
    snapshot_root: PathBuf,
    artifact_root: PathBuf,
    output_root: PathBuf,
    sealed: bool,
}

impl PreparedStaging {
    pub(super) fn create(cache_root: &ArtifactCacheRoot) -> Result<Self, StagingError> {
        let namespace = cache_root.as_path().join(COMPILE_CACHE_NAMESPACE);
        let staging_parent = namespace.join(STAGING_DIRECTORY);
        create_directory(cache_root.as_path())?;
        validate_directory(cache_root.as_path())?;
        ensure_private_directory(&namespace)?;
        ensure_private_directory(&staging_parent)?;
        let root = tempfile::Builder::new()
            .prefix("build-")
            .tempdir_in(&staging_parent)
            .map_err(|source| StagingError::Io {
                operation: StagingIoOperation::CreateDirectory,
                path: staging_parent,
                source,
            })?;
        set_private_directory_permissions(root.path())?;
        let input_root = root.path().join("input");
        let snapshot_root = input_root.join("snapshot");
        let artifact_root = input_root.join("artifacts");
        let output_root = root.path().join("output");
        for path in [&input_root, &snapshot_root, &artifact_root, &output_root] {
            create_directory(path)?;
            set_private_directory_permissions(path)?;
        }
        Ok(Self {
            root,
            input_root,
            snapshot_root,
            artifact_root,
            output_root,
            sealed: false,
        })
    }

    pub(super) fn root(&self) -> &Path {
        self.root.path()
    }

    pub(super) fn output_root(&self) -> &Path {
        &self.output_root
    }

    pub(super) fn planned_output(&self, cone_directory: &str) -> PathBuf {
        self.output_root.join(format!("{cone_directory}.slib"))
    }

    pub(super) fn materialize_manifest(
        &self,
        cone_directory: &str,
        manifest_bytes: &[u8],
        manifest_digest: Digest256,
    ) -> Result<PathBuf, StagingError> {
        let root = self.snapshot_root.join(cone_directory);
        create_directory(&root)?;
        set_private_directory_permissions(&root)?;
        write_verified_file(&root.join("Cone.toml"), manifest_bytes, manifest_digest)?;
        Ok(root)
    }

    pub(super) fn materialize_source(
        &self,
        cone_directory: &str,
        logical_path: &str,
        bytes: &[u8],
        digest: Digest256,
    ) -> Result<PathBuf, StagingError> {
        let mut path = self.snapshot_root.join(cone_directory);
        for component in logical_path.split('/') {
            path.push(component);
        }
        let parent = path
            .parent()
            .ok_or_else(|| StagingError::InvalidInternalPath(path.clone()))?;
        create_directory(parent)?;
        set_private_directory_permissions(parent)?;
        write_verified_file(&path, bytes, digest)?;
        Ok(path)
    }

    pub(super) fn materialize_single_file(
        &self,
        cone_directory: &str,
        bytes: &[u8],
        digest: Digest256,
    ) -> Result<PathBuf, StagingError> {
        let root = self.snapshot_root.join(cone_directory);
        create_directory(&root)?;
        set_private_directory_permissions(&root)?;
        let path = root.join("main.scoop");
        write_verified_file(&path, bytes, digest)?;
        Ok(path)
    }

    pub(super) fn materialize_artifact(
        &self,
        cone_directory: &str,
        candidate_index: usize,
        bytes: &[u8],
        digest: Digest256,
    ) -> Result<PathBuf, StagingError> {
        let root = self.artifact_root.join(cone_directory);
        create_directory(&root)?;
        set_private_directory_permissions(&root)?;
        let path = root.join(format!("{candidate_index:08}.slib"));
        write_verified_file(&path, bytes, digest)?;
        Ok(path)
    }

    pub(super) fn seal_inputs(&mut self) -> Result<(), StagingError> {
        seal_tree(&self.input_root)?;
        self.sealed = true;
        Ok(())
    }
}

impl Drop for PreparedStaging {
    fn drop(&mut self) {
        if self.sealed {
            let _ = unseal_tree(&self.input_root);
        }
    }
}

fn write_verified_file(
    path: &Path,
    bytes: &[u8],
    expected_digest: Digest256,
) -> Result<(), StagingError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| StagingError::Io {
            operation: StagingIoOperation::CreateFile,
            path: path.to_path_buf(),
            source,
        })?;
    file.write_all(bytes).map_err(|source| StagingError::Io {
        operation: StagingIoOperation::Write,
        path: path.to_path_buf(),
        source,
    })?;
    file.sync_all().map_err(|source| StagingError::Io {
        operation: StagingIoOperation::Sync,
        path: path.to_path_buf(),
        source,
    })?;
    drop(file);
    let limit = u64::try_from(bytes.len()).map_err(|_| StagingError::LengthOverflow)?;
    let actual = ImmutableInputSnapshot::capture(path, limit).map_err(StagingError::Verify)?;
    if actual.digest() != expected_digest || actual.as_bytes() != bytes {
        return Err(StagingError::VerificationMismatch(path.to_path_buf()));
    }
    set_read_only_file_permissions(path)
}

fn create_directory(path: &Path) -> Result<(), StagingError> {
    std::fs::create_dir_all(path).map_err(|source| StagingError::Io {
        operation: StagingIoOperation::CreateDirectory,
        path: path.to_path_buf(),
        source,
    })
}

fn ensure_private_directory(path: &Path) -> Result<(), StagingError> {
    match std::fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(source) => {
            return Err(StagingError::Io {
                operation: StagingIoOperation::CreateDirectory,
                path: path.to_path_buf(),
                source,
            });
        }
    }
    validate_directory(path)?;
    set_private_directory_permissions(path)
}

fn validate_directory(path: &Path) -> Result<(), StagingError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|source| StagingError::Io {
        operation: StagingIoOperation::Inspect,
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.file_type().is_dir() {
        return Err(StagingError::UnexpectedFileType(path.to_path_buf()));
    }
    Ok(())
}

fn seal_tree(path: &Path) -> Result<(), StagingError> {
    let mut entries = std::fs::read_dir(path)
        .map_err(|source| StagingError::Io {
            operation: StagingIoOperation::ListDirectory,
            path: path.to_path_buf(),
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| StagingError::Io {
            operation: StagingIoOperation::ListDirectory,
            path: path.to_path_buf(),
            source,
        })?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let file_type = entry.file_type().map_err(|source| StagingError::Io {
            operation: StagingIoOperation::Inspect,
            path: entry.path(),
            source,
        })?;
        if file_type.is_dir() {
            seal_tree(&entry.path())?;
        } else if !file_type.is_file() {
            return Err(StagingError::UnexpectedFileType(entry.path()));
        }
    }
    set_sealed_directory_permissions(path)
}

fn unseal_tree(path: &Path) -> Result<(), StagingError> {
    set_private_directory_permissions(path)?;
    let entries = std::fs::read_dir(path).map_err(|source| StagingError::Io {
        operation: StagingIoOperation::ListDirectory,
        path: path.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| StagingError::Io {
            operation: StagingIoOperation::ListDirectory,
            path: path.to_path_buf(),
            source,
        })?;
        if entry
            .file_type()
            .map_err(|source| StagingError::Io {
                operation: StagingIoOperation::Inspect,
                path: entry.path(),
                source,
            })?
            .is_dir()
        {
            unseal_tree(&entry.path())?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> Result<(), StagingError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(|source| {
        StagingError::Io {
            operation: StagingIoOperation::SetPermissions,
            path: path.to_path_buf(),
            source,
        }
    })
}

#[cfg(not(unix))]
fn set_private_directory_permissions(path: &Path) -> Result<(), StagingError> {
    let mut permissions = std::fs::metadata(path)
        .map_err(|source| StagingError::Io {
            operation: StagingIoOperation::Inspect,
            path: path.to_path_buf(),
            source,
        })?
        .permissions();
    permissions.set_readonly(false);
    std::fs::set_permissions(path, permissions).map_err(|source| StagingError::Io {
        operation: StagingIoOperation::SetPermissions,
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(unix)]
fn set_sealed_directory_permissions(path: &Path) -> Result<(), StagingError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o500)).map_err(|source| {
        StagingError::Io {
            operation: StagingIoOperation::SetPermissions,
            path: path.to_path_buf(),
            source,
        }
    })
}

#[cfg(not(unix))]
fn set_sealed_directory_permissions(path: &Path) -> Result<(), StagingError> {
    let mut permissions = std::fs::metadata(path)
        .map_err(|source| StagingError::Io {
            operation: StagingIoOperation::Inspect,
            path: path.to_path_buf(),
            source,
        })?
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(path, permissions).map_err(|source| StagingError::Io {
        operation: StagingIoOperation::SetPermissions,
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(unix)]
fn set_read_only_file_permissions(path: &Path) -> Result<(), StagingError> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o400)).map_err(|source| {
        StagingError::Io {
            operation: StagingIoOperation::SetPermissions,
            path: path.to_path_buf(),
            source,
        }
    })
}

#[cfg(not(unix))]
fn set_read_only_file_permissions(path: &Path) -> Result<(), StagingError> {
    let mut permissions = std::fs::metadata(path)
        .map_err(|source| StagingError::Io {
            operation: StagingIoOperation::Inspect,
            path: path.to_path_buf(),
            source,
        })?
        .permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(path, permissions).map_err(|source| StagingError::Io {
        operation: StagingIoOperation::SetPermissions,
        path: path.to_path_buf(),
        source,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StagingIoOperation {
    CreateDirectory,
    CreateFile,
    Write,
    Sync,
    ListDirectory,
    Inspect,
    SetPermissions,
}

impl fmt::Display for StagingIoOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CreateDirectory => "create directory",
            Self::CreateFile => "create file",
            Self::Write => "write",
            Self::Sync => "sync",
            Self::ListDirectory => "list directory",
            Self::Inspect => "inspect",
            Self::SetPermissions => "set permissions on",
        })
    }
}

#[derive(Debug)]
pub enum StagingError {
    Io {
        operation: StagingIoOperation,
        path: PathBuf,
        source: std::io::Error,
    },
    Verify(crate::SnapshotFileError),
    LengthOverflow,
    VerificationMismatch(PathBuf),
    InvalidInternalPath(PathBuf),
    UnexpectedFileType(PathBuf),
}

impl fmt::Display for StagingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "cannot {operation} {}: {source}", path.display()),
            Self::Verify(error) => write!(formatter, "cannot verify staged input: {error}"),
            Self::LengthOverflow => formatter.write_str("staged input length does not fit u64"),
            Self::VerificationMismatch(path) => write!(
                formatter,
                "staged input {} does not match its immutable snapshot",
                path.display()
            ),
            Self::InvalidInternalPath(path) => {
                write!(formatter, "invalid private staging path {}", path.display())
            }
            Self::UnexpectedFileType(path) => write!(
                formatter,
                "private staging contains unexpected file type {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for StagingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Verify(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::symlink;

    use super::*;

    #[test]
    fn prepared_staging_rejects_a_symlinked_cache_namespace() {
        let temp = tempfile::tempdir().unwrap();
        let cache_root = temp.path().join("cache");
        let outside = temp.path().join("outside");
        std::fs::create_dir(&cache_root).unwrap();
        std::fs::create_dir(&outside).unwrap();
        symlink(&outside, cache_root.join(COMPILE_CACHE_NAMESPACE)).unwrap();

        assert!(matches!(
            PreparedStaging::create(&ArtifactCacheRoot::new(cache_root).unwrap()),
            Err(StagingError::UnexpectedFileType(path))
                if path.ends_with(COMPILE_CACHE_NAMESPACE)
        ));
    }
}
