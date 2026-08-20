//! Captured regular-file bytes and their content digest.

mod io;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use scoop_wire::{Digest256, sha256};

pub use io::{SnapshotFileError, SnapshotIoOperation};

use io::read_stable_regular_file;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImmutableInputSnapshot {
    source_locator: PathBuf,
    resolved_path: PathBuf,
    bytes: Arc<[u8]>,
    digest: Digest256,
    observation: io::FileObservation,
}

impl ImmutableInputSnapshot {
    pub fn capture(source_locator: &Path) -> Result<Self, SnapshotFileError> {
        let captured = read_stable_regular_file(source_locator)?;
        let bytes: Arc<[u8]> = captured.bytes.into();
        let digest = sha256(&bytes);
        Ok(Self {
            source_locator: source_locator.to_path_buf(),
            resolved_path: captured.resolved_path,
            bytes,
            digest,
            observation: captured.observation,
        })
    }

    pub fn capture_no_follow(source_locator: &Path) -> Result<Self, SnapshotFileError> {
        let captured = io::read_stable_regular_file_no_follow(source_locator)?;
        let bytes: Arc<[u8]> = captured.bytes.into();
        let digest = sha256(&bytes);
        Ok(Self {
            source_locator: source_locator.to_path_buf(),
            resolved_path: captured.resolved_path,
            bytes,
            digest,
            observation: captured.observation,
        })
    }

    pub fn source_locator(&self) -> &Path {
        &self.source_locator
    }

    /// Check whether an external input still has the captured file identity and timestamps.
    pub fn is_current(&self) -> bool {
        self.observation.is_current(&self.source_locator)
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
        assert!(snapshot.is_current());

        std::fs::write(&input, b"changed").unwrap();
        assert!(!snapshot.is_current());
        assert_eq!(snapshot.as_bytes(), b"immutable input");
    }

    #[test]
    fn snapshot_rejects_non_regular_files() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.bin");
        std::fs::write(&input, b"1234").unwrap();

        assert!(matches!(
            ImmutableInputSnapshot::capture(directory.path()),
            Err(SnapshotFileError::NotRegularFile(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn no_follow_snapshot_rejects_a_direct_symlink() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target.bin");
        let alias = directory.path().join("alias.bin");
        std::fs::write(&target, b"target").unwrap();
        symlink(&target, &alias).unwrap();

        assert!(matches!(
            ImmutableInputSnapshot::capture_no_follow(&alias),
            Err(SnapshotFileError::NotRegularFile(path)) if path == alias
        ));
    }
}
