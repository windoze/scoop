use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};

use scoop_identity::SourceIdentity;

use crate::{DiscoveredSource, SelectedSource, SourceDisplayLocator};

mod header;
pub use header::{SingleFileDependency, SingleFileHeaderError, parse_single_file_dependencies};

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
    Write,
    Lock,
}

impl fmt::Display for SingleFileInputIoOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Canonicalize => "canonicalize",
            Self::Inspect => "inspect",
            Self::Read => "read",
            Self::Write => "write",
            Self::Lock => "lock",
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

    pub fn io(
        operation: SingleFileInputIoOperation,
        path: PathBuf,
        source: std::io::Error,
    ) -> Self {
        Self::new(path, SingleFileInputErrorKind::Io { operation, source })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn kind(&self) -> &SingleFileInputErrorKind {
        &self.kind
    }

    pub fn header(path: PathBuf, error: SingleFileHeaderError) -> Self {
        Self::new(path, SingleFileInputErrorKind::Header(error))
    }

    pub fn span(&self) -> Option<std::ops::Range<usize>> {
        match &self.kind {
            SingleFileInputErrorKind::Header(error) => Some(error.span.clone()),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum SingleFileInputErrorKind {
    InvalidExtension,
    NotRegularFile,
    InvalidUtf8,
    Header(SingleFileHeaderError),

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
            SingleFileInputErrorKind::Header(error) => write!(
                formatter,
                "{}..{}: {error}",
                error.span.start, error.span.end
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

pub fn select_single_file_source(
    locator: &SingleFileLocator,
) -> Result<SelectedSource, SingleFileInputError> {
    let metadata = std::fs::metadata(locator.resolved_path()).map_err(|error| {
        SingleFileInputError::io(
            SingleFileInputIoOperation::Inspect,
            locator.display_path.clone(),
            error,
        )
    })?;
    SelectedSource::new(
        SourceIdentity::single_file(),
        SourceDisplayLocator::new(locator.display_path.clone()),
        locator.resolved_path.clone(),
        &metadata,
        scoop_identity::SourceRole::Production,
    )
    .map_err(|error| {
        SingleFileInputError::io(
            SingleFileInputIoOperation::Inspect,
            locator.display_path.clone(),
            error,
        )
    })
}

pub fn load_single_file_source(
    locator: &SingleFileLocator,
) -> Result<DiscoveredSource, SingleFileInputError> {
    let bytes = std::fs::read(locator.resolved_path()).map_err(|error| {
        SingleFileInputError::io(
            SingleFileInputIoOperation::Read,
            locator.display_path.clone(),
            error,
        )
    })?;
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
}
