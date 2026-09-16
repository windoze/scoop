//! Immutable input snapshots used by graph preflight.
//!
//! These values prove only that exact regular-file bytes were captured from a
//! stable locator. They deliberately grant no artifact Graph, Compile, Link,
//! publication, or child-launch authority.

mod io;
mod prepared;
mod staging;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use scoop_wire::{Digest256, sha256};

pub use io::{SnapshotFileError, SnapshotIoOperation};
pub use prepared::*;
pub use staging::{StagingError, StagingIoOperation};

use io::read_stable_regular_file;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImmutableInputSnapshot {
    source_locator: PathBuf,
    resolved_path: PathBuf,
    bytes: Arc<[u8]>,
    digest: Digest256,
}

impl ImmutableInputSnapshot {
    pub fn capture(source_locator: &Path, byte_limit: u64) -> Result<Self, SnapshotFileError> {
        let captured = read_stable_regular_file(source_locator, byte_limit)?;
        let bytes: Arc<[u8]> = captured.bytes.into();
        let digest = sha256(&bytes);
        Ok(Self {
            source_locator: source_locator.to_path_buf(),
            resolved_path: captured.resolved_path,
            bytes,
            digest,
        })
    }

    pub(crate) fn capture_no_follow(
        source_locator: &Path,
        byte_limit: u64,
    ) -> Result<Self, SnapshotFileError> {
        let captured = io::read_stable_regular_file_no_follow(source_locator, byte_limit)?;
        let bytes: Arc<[u8]> = captured.bytes.into();
        let digest = sha256(&bytes);
        Ok(Self {
            source_locator: source_locator.to_path_buf(),
            resolved_path: captured.resolved_path,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_owns_exact_bytes_and_digest() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.bin");
        std::fs::write(&input, b"immutable input").unwrap();

        let snapshot = ImmutableInputSnapshot::capture(&input, 15).unwrap();

        assert_eq!(snapshot.source_locator(), input);
        assert_eq!(snapshot.resolved_path(), input.canonicalize().unwrap());
        assert_eq!(snapshot.as_bytes(), b"immutable input");
        assert_eq!(snapshot.digest(), sha256(b"immutable input"));

        std::fs::write(&input, b"changed").unwrap();
        assert_eq!(snapshot.as_bytes(), b"immutable input");
    }

    #[test]
    fn snapshot_enforces_regular_file_and_size_before_growth() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.bin");
        std::fs::write(&input, b"1234").unwrap();

        assert!(matches!(
            ImmutableInputSnapshot::capture(&input, 3),
            Err(SnapshotFileError::TooLarge {
                limit: 3,
                observed: 4,
                ..
            })
        ));
        assert!(matches!(
            ImmutableInputSnapshot::capture(directory.path(), u64::MAX),
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
            ImmutableInputSnapshot::capture_no_follow(&alias, 6),
            Err(SnapshotFileError::NotRegularFile(path)) if path == alias
        ));
    }
}
