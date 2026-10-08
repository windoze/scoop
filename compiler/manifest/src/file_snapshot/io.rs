use std::fmt;
use std::fs::{File, Metadata};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug)]
pub(super) struct StableFileBytes {
    pub(super) resolved_path: PathBuf,
    pub(super) bytes: Vec<u8>,
    pub(super) observation: FileObservation,
}

pub(super) fn read_stable_regular_file(
    locator: &Path,
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
        observation,
    })
}

pub(super) fn read_stable_regular_file_no_follow(
    locator: &Path,
) -> Result<StableFileBytes, SnapshotFileError> {
    let before_path =
        std::fs::symlink_metadata(locator).map_err(|source| SnapshotFileError::Io {
            operation: SnapshotIoOperation::Inspect,
            path: locator.to_path_buf(),
            source,
        })?;
    if !before_path.file_type().is_file() {
        return Err(SnapshotFileError::NotRegularFile(locator.to_path_buf()));
    }
    let mut file = open_no_follow(locator).map_err(|source| SnapshotFileError::Io {
        operation: SnapshotIoOperation::Open,
        path: locator.to_path_buf(),
        source,
    })?;
    let before = file.metadata().map_err(|source| SnapshotFileError::Io {
        operation: SnapshotIoOperation::Inspect,
        path: locator.to_path_buf(),
        source,
    })?;
    let observation = FileObservation::new(&before);
    if !before.is_file() || FileObservation::new(&before_path) != observation {
        return Err(SnapshotFileError::Changed {
            locator: locator.to_path_buf(),
            before: locator.to_path_buf(),
            after: locator.to_path_buf(),
        });
    }

    let capacity =
        usize::try_from(observation.len).map_err(|_| SnapshotFileError::LengthOverflow {
            path: locator.to_path_buf(),
        })?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(capacity)
        .map_err(|_| SnapshotFileError::Allocation {
            path: locator.to_path_buf(),
            requested_bytes: observation.len,
        })?;
    let read_limit =
        observation
            .len
            .checked_add(1)
            .ok_or_else(|| SnapshotFileError::LengthOverflow {
                path: locator.to_path_buf(),
            })?;
    file.by_ref()
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|source| SnapshotFileError::Io {
            operation: SnapshotIoOperation::Read,
            path: locator.to_path_buf(),
            source,
        })?;
    let after = file.metadata().map_err(|source| SnapshotFileError::Io {
        operation: SnapshotIoOperation::Inspect,
        path: locator.to_path_buf(),
        source,
    })?;
    let after_path =
        std::fs::symlink_metadata(locator).map_err(|source| SnapshotFileError::Io {
            operation: SnapshotIoOperation::Inspect,
            path: locator.to_path_buf(),
            source,
        })?;
    if !after_path.file_type().is_file()
        || FileObservation::new(&after) != observation
        || FileObservation::new(&after_path) != observation
        || u64::try_from(bytes.len()).ok() != Some(observation.len)
    {
        return Err(SnapshotFileError::Changed {
            locator: locator.to_path_buf(),
            before: locator.to_path_buf(),
            after: locator.to_path_buf(),
        });
    }
    Ok(StableFileBytes {
        resolved_path: locator.to_path_buf(),
        bytes,
        observation,
    })
}

#[cfg(unix)]
fn open_no_follow(path: &Path) -> std::io::Result<File> {
    use rustix::fs::{Mode, OFlags};

    rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(std::io::Error::from)
}

#[cfg(not(unix))]
fn open_no_follow(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FileObservation {
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
    pub(super) fn is_current(&self, path: &Path) -> bool {
        std::fs::metadata(path)
            .is_ok_and(|metadata| metadata.is_file() && Self::new(&metadata) == *self)
    }

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
