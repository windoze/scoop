use super::*;

pub(super) fn read_discovered_source(
    candidate: SourceCandidate,
    cone: scoop_identity::ConeIdentity,
) -> Result<DiscoveredSource, SourceDiscoveryError> {
    let bytes = std::fs::read(&candidate.physical_path).map_err(|error| {
        SourceDiscoveryError::io(
            DiscoveryIoOperation::ReadSource,
            candidate.display_path.clone(),
            error,
        )
    })?;
    let source_text = String::from_utf8(bytes).map_err(|_| {
        SourceDiscoveryError::new(
            candidate.display_path.clone(),
            SourceDiscoveryErrorKind::InvalidUtf8Source,
        )
    })?;
    let identity = SourceIdentity::new(cone, candidate.logical_path).map_err(|error| {
        SourceDiscoveryError::new(
            candidate.display_path.clone(),
            SourceDiscoveryErrorKind::InvalidSourceIdentity(error.to_string()),
        )
    })?;
    Ok(DiscoveredSource::new(
        identity,
        SourceDisplayLocator::new(candidate.display_path),
        source_text,
    ))
}
