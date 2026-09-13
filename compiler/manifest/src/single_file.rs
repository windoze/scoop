use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};

use scoop_identity::SourceIdentity;

use crate::{DiscoveredSource, SourceDisplayLocator};

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

pub fn load_single_file_source(path: &Path) -> Result<DiscoveredSource, SingleFileInputError> {
    if path.extension() != Some(OsStr::new("scoop")) {
        return Err(SingleFileInputError::new(
            path.to_path_buf(),
            SingleFileInputErrorKind::InvalidExtension,
        ));
    }

    let canonical = std::fs::canonicalize(path).map_err(|error| {
        SingleFileInputError::io(
            SingleFileInputIoOperation::Canonicalize,
            path.to_path_buf(),
            error,
        )
    })?;
    if !std::fs::metadata(&canonical)
        .map_err(|error| {
            SingleFileInputError::io(
                SingleFileInputIoOperation::Inspect,
                path.to_path_buf(),
                error,
            )
        })?
        .is_file()
    {
        return Err(SingleFileInputError::new(
            path.to_path_buf(),
            SingleFileInputErrorKind::NotRegularFile,
        ));
    }
    let bytes = std::fs::read(&canonical).map_err(|error| {
        SingleFileInputError::io(SingleFileInputIoOperation::Read, path.to_path_buf(), error)
    })?;
    let after_read = std::fs::canonicalize(path).map_err(|error| {
        SingleFileInputError::io(
            SingleFileInputIoOperation::Canonicalize,
            path.to_path_buf(),
            error,
        )
    })?;
    let after_read_metadata = std::fs::metadata(&after_read).map_err(|error| {
        SingleFileInputError::io(
            SingleFileInputIoOperation::Inspect,
            path.to_path_buf(),
            error,
        )
    })?;
    if canonical != after_read || !after_read_metadata.is_file() {
        return Err(SingleFileInputError::new(
            path.to_path_buf(),
            SingleFileInputErrorKind::SourceChangedDuringRead,
        ));
    }
    let source_text = String::from_utf8(bytes).map_err(|_| {
        SingleFileInputError::new(path.to_path_buf(), SingleFileInputErrorKind::InvalidUtf8)
    })?;

    Ok(DiscoveredSource::new(
        SourceIdentity::single_file(),
        SourceDisplayLocator::new(path.to_path_buf()),
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

        let source = load_single_file_source(&path).unwrap();
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
            load_single_file_source(&path).unwrap_err().kind(),
            SingleFileInputErrorKind::InvalidExtension
        ));
    }
}
