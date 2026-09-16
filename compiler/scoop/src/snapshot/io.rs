use std::fmt;
use std::fs::{File, Metadata};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug)]
pub(super) struct StableFileBytes {
    pub(super) resolved_path: PathBuf,
    pub(super) bytes: Vec<u8>,
}

pub(super) fn read_stable_regular_file(
    locator: &Path,
    byte_limit: u64,
) -> Result<StableFileBytes, SnapshotFileError> {
    let resolved_path = std::fs::canonicalize(locator).map_err(|source| SnapshotFileError::Io {
        operation: SnapshotIoOperation::Canonicalize,
        path: locator.to_path_buf(),
        source,
    })?;
    let mut file = File::open(&resolved_path).map_err(|source| SnapshotFileError::Io {
        operation: SnapshotIoOperation::Open,
        path: resolved_path.clone(),
        source,
    })?;
    let before = file.metadata().map_err(|source| SnapshotFileError::Io {
        operation: SnapshotIoOperation::Inspect,
        path: resolved_path.clone(),
        source,
    })?;
    if !before.is_file() {
        return Err(SnapshotFileError::NotRegularFile(resolved_path));
    }
    let observation = FileObservation::new(&before);
    if observation.len > byte_limit {
        return Err(SnapshotFileError::TooLarge {
            path: resolved_path,
            limit: byte_limit,
            observed: observation.len,
        });
    }
    let capacity =
        usize::try_from(observation.len).map_err(|_| SnapshotFileError::LengthOverflow {
            path: resolved_path.clone(),
        })?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| SnapshotFileError::Allocation {
            path: resolved_path.clone(),
            requested_bytes: observation.len,
        })?;
    let read_limit =
        observation
            .len
            .checked_add(1)
            .ok_or_else(|| SnapshotFileError::LengthOverflow {
                path: resolved_path.clone(),
            })?;
    file.by_ref()
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|source| SnapshotFileError::Io {
            operation: SnapshotIoOperation::Read,
            path: resolved_path.clone(),
            source,
        })?;
    let after = file.metadata().map_err(|source| SnapshotFileError::Io {
        operation: SnapshotIoOperation::Inspect,
        path: resolved_path.clone(),
        source,
    })?;
    let after_resolved =
        std::fs::canonicalize(locator).map_err(|source| SnapshotFileError::Io {
            operation: SnapshotIoOperation::Canonicalize,
            path: locator.to_path_buf(),
            source,
        })?;
    let after_path =
        std::fs::metadata(&after_resolved).map_err(|source| SnapshotFileError::Io {
            operation: SnapshotIoOperation::Inspect,
            path: after_resolved.clone(),
            source,
        })?;
    if after_resolved != resolved_path
        || !after.is_file()
        || !after_path.is_file()
        || FileObservation::new(&after) != observation
        || FileObservation::new(&after_path) != observation
        || u64::try_from(bytes.len()).ok() != Some(observation.len)
    {
        return Err(SnapshotFileError::Changed {
            locator: locator.to_path_buf(),
            before: resolved_path,
            after: after_resolved,
        });
    }
    Ok(StableFileBytes {
        resolved_path,
        bytes,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileObservation {
    len: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    modified_seconds: i64,
    #[cfg(unix)]
    modified_nanoseconds: i64,
    #[cfg(unix)]
    changed_seconds: i64,
    #[cfg(unix)]
    changed_nanoseconds: i64,
}

impl FileObservation {
    fn new(metadata: &Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;

        Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
            #[cfg(unix)]
            modified_seconds: metadata.mtime(),
            #[cfg(unix)]
            modified_nanoseconds: metadata.mtime_nsec(),
            #[cfg(unix)]
            changed_seconds: metadata.ctime(),
            #[cfg(unix)]
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotIoOperation {
    Canonicalize,
    Open,
    Inspect,
    Read,
}

impl fmt::Display for SnapshotIoOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Canonicalize => "canonicalize",
            Self::Open => "open",
            Self::Inspect => "inspect",
            Self::Read => "read",
        })
    }
}

#[derive(Debug)]
pub enum SnapshotFileError {
    Io {
        operation: SnapshotIoOperation,
        path: PathBuf,
        source: std::io::Error,
    },
    NotRegularFile(PathBuf),
    TooLarge {
        path: PathBuf,
        limit: u64,
        observed: u64,
    },
    LengthOverflow {
        path: PathBuf,
    },
    Allocation {
        path: PathBuf,
        requested_bytes: u64,
    },
    Changed {
        locator: PathBuf,
        before: PathBuf,
        after: PathBuf,
    },
}

impl fmt::Display for SnapshotFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "cannot {operation} {}: {source}", path.display()),
            Self::NotRegularFile(path) => {
                write!(
                    formatter,
                    "snapshot input {} is not a regular file",
                    path.display()
                )
            }
            Self::TooLarge {
                path,
                limit,
                observed,
            } => write!(
                formatter,
                "snapshot input {} exceeds byte limit {limit}: observed {observed}",
                path.display()
            ),
            Self::LengthOverflow { path } => write!(
                formatter,
                "snapshot input length does not fit the bounded reader: {}",
                path.display()
            ),
            Self::Allocation {
                path,
                requested_bytes,
            } => write!(
                formatter,
                "cannot allocate {requested_bytes} bytes for snapshot input {}",
                path.display()
            ),
            Self::Changed {
                locator,
                before,
                after,
            } => write!(
                formatter,
                "snapshot input {} changed from {} to {} while it was read",
                locator.display(),
                before.display(),
                after.display()
            ),
        }
    }
}

impl std::error::Error for SnapshotFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
