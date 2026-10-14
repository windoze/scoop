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
    let temporary = temporary.into_temp_path();
    temporary.persist(destination).map_err(|error| {
        CrossConeArtifactPublishError::io(
            CrossConePublishIoOperation::RenameTemporary,
            destination.to_path_buf(),
            error.error,
        )
    })?;

    Ok(PublishedCrossConeArtifact {
        path: destination.to_path_buf(),
        summary,
    })
}
