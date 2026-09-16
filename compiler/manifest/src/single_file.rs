use std::ffi::OsStr;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use scoop_identity::SourceIdentity;

use crate::{DiscoveredSource, SourceDisplayLocator};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SingleFileLocator {
    display_path: PathBuf,
    resolved_path: PathBuf,
}

impl SingleFileLocator {
    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self, SingleFileInputError> {
        let path = path.into();
        if path.extension() != Some(OsStr::new("scoop")) {
            return Err(SingleFileInputError::new(
                path,
                SingleFileInputErrorKind::InvalidExtension,
            ));
        }

        let resolved_path = std::fs::canonicalize(&path).map_err(|error| {
            SingleFileInputError::io(
                SingleFileInputIoOperation::Canonicalize,
                path.clone(),
                error,
            )
        })?;
        if !std::fs::metadata(&resolved_path)
            .map_err(|error| {
                SingleFileInputError::io(SingleFileInputIoOperation::Inspect, path.clone(), error)
            })?
            .is_file()
        {
            return Err(SingleFileInputError::new(
                path,
                SingleFileInputErrorKind::NotRegularFile,
            ));
        }

        Ok(Self {
            display_path: path,
            resolved_path,
        })
    }

    pub fn display_path(&self) -> &Path {
        &self.display_path
    }

    pub fn resolved_path(&self) -> &Path {
        &self.resolved_path
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SingleFileInputIoOperation {
    Canonicalize,
    Inspect,
    Read,
}

impl fmt::Display for SingleFileInputIoOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Canonicalize => "canonicalize",
            Self::Inspect => "inspect",
            Self::Read => "read",
        })
    }
}

#[derive(Debug)]
pub struct SingleFileInputError {
    path: PathBuf,
    kind: SingleFileInputErrorKind,
}

impl SingleFileInputError {
    fn new(path: PathBuf, kind: SingleFileInputErrorKind) -> Self {
        Self { path, kind }
    }

    fn io(operation: SingleFileInputIoOperation, path: PathBuf, source: std::io::Error) -> Self {
        Self::new(path, SingleFileInputErrorKind::Io { operation, source })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn kind(&self) -> &SingleFileInputErrorKind {
        &self.kind
    }
}

#[derive(Debug)]
pub enum SingleFileInputErrorKind {
    InvalidExtension,
    NotRegularFile,
    InvalidUtf8,
    SourceChangedDuringRead,
    ByteLimitExceeded {
        limit: u64,
        observed: u64,
    },
    LengthOverflow,
    Allocation {
        requested_bytes: u64,
    },
    Io {
        operation: SingleFileInputIoOperation,
        source: std::io::Error,
    },
}

impl fmt::Display for SingleFileInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid single-file input {}: ",
            self.path.display()
        )?;
        match &self.kind {
            SingleFileInputErrorKind::InvalidExtension => {
                formatter.write_str("operand extension must be exactly .scoop")
            }
            SingleFileInputErrorKind::NotRegularFile => {
                formatter.write_str("resolved input is not a regular file")
            }
            SingleFileInputErrorKind::InvalidUtf8 => {
                formatter.write_str("source file is not valid UTF-8")
            }
            SingleFileInputErrorKind::SourceChangedDuringRead => {
                formatter.write_str("source target changed while it was read")
            }
            SingleFileInputErrorKind::ByteLimitExceeded { limit, observed } => write!(
                formatter,
                "source bytes exceed limit {limit}: observed {observed}"
            ),
            SingleFileInputErrorKind::LengthOverflow => {
                formatter.write_str("source length does not fit the bounded reader")
            }
            SingleFileInputErrorKind::Allocation { requested_bytes } => write!(
                formatter,
                "cannot allocate {requested_bytes} bytes for single-file source"
            ),
            SingleFileInputErrorKind::Io { operation, source } => {
                write!(formatter, "cannot {operation}: {source}")
            }
        }
    }
}

impl std::error::Error for SingleFileInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            SingleFileInputErrorKind::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub fn load_single_file_source(
    locator: &SingleFileLocator,
) -> Result<DiscoveredSource, SingleFileInputError> {
    load_single_file_source_with_limit(locator, u64::MAX)
}

pub fn load_single_file_source_with_limit(
    locator: &SingleFileLocator,
    byte_limit: u64,
) -> Result<DiscoveredSource, SingleFileInputError> {
    let mut file = File::open(locator.resolved_path()).map_err(|error| {
        SingleFileInputError::io(
            SingleFileInputIoOperation::Read,
            locator.display_path.clone(),
            error,
        )
    })?;
    let before = file.metadata().map_err(|error| {
        SingleFileInputError::io(
            SingleFileInputIoOperation::Inspect,
            locator.display_path.clone(),
            error,
        )
    })?;
    if !before.is_file() {
        return Err(SingleFileInputError::new(
            locator.display_path.clone(),
            SingleFileInputErrorKind::SourceChangedDuringRead,
        ));
    }
    let expected_length = before.len();
    if expected_length > byte_limit {
        return Err(SingleFileInputError::new(
            locator.display_path.clone(),
            SingleFileInputErrorKind::ByteLimitExceeded {
                limit: byte_limit,
                observed: expected_length,
            },
        ));
    }
    let capacity = usize::try_from(expected_length).map_err(|_| {
        SingleFileInputError::new(
            locator.display_path.clone(),
            SingleFileInputErrorKind::LengthOverflow,
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity).map_err(|_| {
        SingleFileInputError::new(
            locator.display_path.clone(),
            SingleFileInputErrorKind::Allocation {
                requested_bytes: expected_length,
            },
        )
    })?;
    let read_limit = expected_length.checked_add(1).ok_or_else(|| {
        SingleFileInputError::new(
            locator.display_path.clone(),
            SingleFileInputErrorKind::LengthOverflow,
        )
    })?;
    file.by_ref()
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            SingleFileInputError::io(
                SingleFileInputIoOperation::Read,
                locator.display_path.clone(),
                error,
            )
        })?;
    let after_file = file.metadata().map_err(|error| {
        SingleFileInputError::io(
            SingleFileInputIoOperation::Inspect,
            locator.display_path.clone(),
            error,
        )
    })?;
    let after_read = std::fs::canonicalize(locator.display_path()).map_err(|error| {
        SingleFileInputError::io(
            SingleFileInputIoOperation::Canonicalize,
            locator.display_path.clone(),
            error,
        )
    })?;
    let after_read_metadata = std::fs::metadata(&after_read).map_err(|error| {
        SingleFileInputError::io(
            SingleFileInputIoOperation::Inspect,
            locator.display_path.clone(),
            error,
        )
    })?;
    if locator.resolved_path != after_read
        || !after_read_metadata.is_file()
        || !after_file.is_file()
        || after_file.len() != expected_length
        || u64::try_from(bytes.len()).ok() != Some(expected_length)
    {
        return Err(SingleFileInputError::new(
            locator.display_path.clone(),
            SingleFileInputErrorKind::SourceChangedDuringRead,
        ));
    }
    let source_text = String::from_utf8(bytes).map_err(|_| {
        SingleFileInputError::new(
            locator.display_path.clone(),
            SingleFileInputErrorKind::InvalidUtf8,
        )
    })?;

    Ok(DiscoveredSource::new(
        SourceIdentity::single_file(),
        SourceDisplayLocator::new(locator.display_path.clone()),
        source_text,
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempDirectory(PathBuf);

    impl TempDirectory {
        fn new() -> Self {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "scoop-single-file-input-{}-{serial}",
                std::process::id()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn assigns_the_reserved_identity_independent_of_operand_name() {
        let directory = TempDirectory::new();
        let path = directory.0.join("different.scoop");
        std::fs::write(&path, "fun main() {}\n").unwrap();

        let locator = SingleFileLocator::from_path(&path).unwrap();
        let source = load_single_file_source(&locator).unwrap();
        assert_eq!(source.identity(), &SourceIdentity::single_file());
        assert_eq!(source.source_text(), "fun main() {}\n");
        assert_eq!(source.display_locator().as_path(), path);
    }

    #[test]
    fn rejects_non_scoop_operand_even_when_the_target_is_text() {
        let directory = TempDirectory::new();
        let path = directory.0.join("main.SCOOP");
        std::fs::write(&path, "fun main() {}\n").unwrap();

        assert!(matches!(
            SingleFileLocator::from_path(&path).unwrap_err().kind(),
            SingleFileInputErrorKind::InvalidExtension
        ));
    }

    #[test]
    fn bounded_single_file_read_has_inclusive_byte_limit() {
        let directory = TempDirectory::new();
        let path = directory.0.join("main.scoop");
        std::fs::write(&path, "abc").unwrap();
        let locator = SingleFileLocator::from_path(&path).unwrap();

        assert_eq!(
            load_single_file_source_with_limit(&locator, 3)
                .unwrap()
                .source_text(),
            "abc"
        );
        assert!(matches!(
            load_single_file_source_with_limit(&locator, 2)
                .unwrap_err()
                .kind(),
            SingleFileInputErrorKind::ByteLimitExceeded {
                limit: 2,
                observed: 3
            }
        ));
    }
}
