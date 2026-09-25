//! Shared final-file validation and atomic replacement.

use std::io::Write;
use std::path::Path;

use super::{
    CrossConeArtifactPublishError, CrossConePublishIoOperation, PublishableCrossConeArtifact,
    PublishedCrossConeArtifact,
};

pub(super) fn publish(
    final_bytes: &[u8],
    destination: &Path,
    validate: impl FnOnce(&[u8]) -> Result<PublishableCrossConeArtifact, CrossConeArtifactPublishError>,
) -> Result<PublishedCrossConeArtifact, CrossConeArtifactPublishError> {
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| CrossConeArtifactPublishError::MissingParent {
            destination: destination.to_path_buf(),
        })?;
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
    temporary.flush().map_err(|source| {
        CrossConeArtifactPublishError::io(
            CrossConePublishIoOperation::FlushTemporary,
            temporary.path().to_path_buf(),
            source,
        )
    })?;
    temporary.as_file().sync_all().map_err(|source| {
        CrossConeArtifactPublishError::io(
            CrossConePublishIoOperation::SyncTemporary,
            temporary.path().to_path_buf(),
            source,
        )
    })?;

    let temporary = temporary.into_temp_path();
    let round_trip_bytes = std::fs::read(&temporary).map_err(|source| {
        CrossConeArtifactPublishError::io(
            CrossConePublishIoOperation::ReadTemporary,
            temporary.to_path_buf(),
            source,
        )
    })?;
    let validation = validate(&round_trip_bytes)?;
    temporary.persist(destination).map_err(|error| {
        CrossConeArtifactPublishError::io(
            CrossConePublishIoOperation::RenameTemporary,
            destination.to_path_buf(),
            error.error,
        )
    })?;

    Ok(PublishedCrossConeArtifact {
        path: destination.to_path_buf(),
        validation,
    })
}
