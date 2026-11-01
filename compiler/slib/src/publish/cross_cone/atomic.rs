//! Write and atomically publish the completed archive.

use std::io::Write;
use std::path::Path;

use super::{
    CrossConeArtifactPublishError, CrossConeArtifactSummary, CrossConePublishIoOperation,
    PublishedCrossConeArtifact,
};

pub(super) fn publish(
    final_bytes: &[u8],
    destination: &Path,
    summary: CrossConeArtifactSummary,
) -> Result<PublishedCrossConeArtifact, CrossConeArtifactPublishError> {
    write(final_bytes, destination)?;
    Ok(PublishedCrossConeArtifact {
        path: destination.to_path_buf(),
        summary,
    })
}

fn write(final_bytes: &[u8], destination: &Path) -> Result<(), CrossConeArtifactPublishError> {
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| CrossConeArtifactPublishError::MissingParent {
            destination: destination.to_path_buf(),
        })?;
    // tempfile's native Windows persistence calls need the long-path form for
    // both names, including when only the longer temporary name exceeds MAX_PATH.
    #[cfg(windows)]
    let canonical_parent = std::fs::canonicalize(parent).map_err(|source| {
        CrossConeArtifactPublishError::io(
            CrossConePublishIoOperation::CreateTemporary,
            parent.to_path_buf(),
            source,
        )
    })?;
    #[cfg(windows)]
    let parent = canonical_parent.as_path();
    #[cfg(windows)]
    let destination_for_io = parent.join(destination.file_name().ok_or_else(|| {
        CrossConeArtifactPublishError::MissingParent {
            destination: destination.to_path_buf(),
        }
    })?);
    #[cfg(not(windows))]
    let destination_for_io = destination;

    let mut temporary = tempfile::Builder::new()
        .prefix(".scoop-publish-")
        .suffix(".slib.tmp")
        .tempfile_in(parent)
        .map_err(|source| {
            CrossConeArtifactPublishError::io(
                CrossConePublishIoOperation::CreateTemporary,
                parent.to_path_buf(),
                source,
            )
        })?;
    temporary.write_all(final_bytes).map_err(|source| {
        CrossConeArtifactPublishError::io(
            CrossConePublishIoOperation::WriteTemporary,
            temporary.path().to_path_buf(),
            source,
        )
    })?;
    let temporary = temporary.into_temp_path();
    temporary.persist(destination_for_io).map_err(|error| {
        CrossConeArtifactPublishError::io(
            CrossConePublishIoOperation::RenameTemporary,
            destination.to_path_buf(),
            error.error,
        )
    })?;

    Ok(())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn long_paths_publish_replace_and_preserve_failed_destinations() {
        let root = tempfile::Builder::new()
            .prefix(".scoop-publication-")
            .tempdir_in(".")
            .unwrap();
        let mut parent = root.path().to_path_buf();
        for _ in 0..5 {
            parent.push("long-publication-directory-".repeat(3));
        }
        std::fs::create_dir_all(&parent).unwrap();
        let destination = parent.join("artifact.slib");
        write(b"first complete archive", &destination).unwrap();
        write(b"replacement complete archive", &destination).unwrap();
        assert_eq!(
            std::fs::read(&destination).unwrap(),
            b"replacement complete archive"
        );

        let occupied = parent.join("occupied");
        std::fs::create_dir(&occupied).unwrap();
        let existing = occupied.join("existing");
        std::fs::write(&existing, b"retained").unwrap();
        assert!(matches!(
            write(b"failed publication", &occupied),
            Err(CrossConeArtifactPublishError::Io {
                operation: CrossConePublishIoOperation::RenameTemporary,
                ..
            })
        ));
        assert_eq!(std::fs::read(existing).unwrap(), b"retained");
        assert_eq!(std::fs::read_dir(parent).unwrap().count(), 2);
    }
}
