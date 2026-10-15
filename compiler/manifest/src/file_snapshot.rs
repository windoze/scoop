//! File contents shared by consumers within the current build.

use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use scoop_wire::{Digest256, sha256};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImmutableInputSnapshot {
    source_locator: PathBuf,
    resolved_path: PathBuf,
    bytes: Arc<[u8]>,
    digest: Digest256,
}

impl ImmutableInputSnapshot {
    pub fn capture(source_locator: &Path) -> Result<Self, SnapshotFileError> {
        let resolved_path =
            std::fs::canonicalize(source_locator).map_err(|source| SnapshotFileError {
                operation: SnapshotIoOperation::Canonicalize,
                path: source_locator.to_path_buf(),
                source,
            })?;
        let bytes: Arc<[u8]> = std::fs::read(&resolved_path)
            .map_err(|source| SnapshotFileError {
                operation: SnapshotIoOperation::Read,
                path: source_locator.to_path_buf(),
                source,
            })?
            .into();
        let digest = sha256(&bytes);
        Ok(Self {
            source_locator: source_locator.to_path_buf(),
            resolved_path,
            bytes,
            digest,
        })
    }

    pub fn source_locator(&self) -> &Path {
        &self.source_locator
    }

    pub fn resolved_path(&self) -> &Path {
        &self.resolved_path
    }

    pub fn byte_length(&self) -> usize {
        self.bytes.len()
    }

    pub const fn digest(&self) -> Digest256 {
        self.digest
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn shared_bytes(&self) -> Arc<[u8]> {
        Arc::clone(&self.bytes)
    }

    pub fn write_to(&self, writer: &mut impl Write) -> std::io::Result<()> {
        writer.write_all(self.as_bytes())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnapshotIoOperation {
    Canonicalize,
    Read,
}

impl fmt::Display for SnapshotIoOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Canonicalize => "canonicalize",
            Self::Read => "read",
        })
    }
}

#[derive(Debug)]
pub struct SnapshotFileError {
    operation: SnapshotIoOperation,
    path: PathBuf,
    source: std::io::Error,
}

impl fmt::Display for SnapshotFileError {
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

impl std::error::Error for SnapshotFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_owns_exact_bytes_and_digest() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.bin");
        std::fs::write(&input, b"immutable input").unwrap();

        let snapshot = ImmutableInputSnapshot::capture(&input).unwrap();

        assert_eq!(snapshot.source_locator(), input);
        assert_eq!(snapshot.resolved_path(), input.canonicalize().unwrap());
        assert_eq!(snapshot.as_bytes(), b"immutable input");
        assert_eq!(snapshot.digest(), sha256(b"immutable input"));
    }

    #[test]
    fn snapshot_reports_an_actual_read_failure() {
        let directory = tempfile::tempdir().unwrap();
        let error = ImmutableInputSnapshot::capture(directory.path()).unwrap_err();
        assert_eq!(error.path, directory.path());
        assert_eq!(error.operation, SnapshotIoOperation::Read);
    }

    #[cfg(unix)]
    #[test]
    fn snapshot_reads_through_an_input_symlink() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target.bin");
        let alias = directory.path().join("alias.bin");
        std::fs::write(&target, b"target").unwrap();
        symlink(&target, &alias).unwrap();

        let snapshot = ImmutableInputSnapshot::capture(&alias).unwrap();
        assert_eq!(snapshot.source_locator(), alias);
        assert_eq!(snapshot.resolved_path(), target.canonicalize().unwrap());
        assert_eq!(snapshot.as_bytes(), b"target");
    }
}
