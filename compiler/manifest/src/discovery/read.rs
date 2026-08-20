use super::*;

pub(super) fn read_discovered_source(
    candidate: SourceCandidate,
    cone: scoop_identity::ConeIdentity,
) -> Result<DiscoveredSource, SourceDiscoveryError> {
    let mut file = File::open(&candidate.physical_path).map_err(|error| {
        SourceDiscoveryError::io(
            DiscoveryIoOperation::ReadSource,
            candidate.display_path.clone(),
            error,
        )
    })?;
    let before = file.metadata().map_err(|error| {
        SourceDiscoveryError::io(
            DiscoveryIoOperation::Inspect,
            candidate.display_path.clone(),
            error,
        )
    })?;
    if !before.is_file() {
        return Err(SourceDiscoveryError::new(
            candidate.display_path,
            SourceDiscoveryErrorKind::SourceChangedDuringDiscovery,
        ));
    }
    let before = StableFileObservation::new(&before);
    let expected_length = before.length();

    let capacity = usize::try_from(expected_length).map_err(|_| {
        SourceDiscoveryError::new(
            candidate.display_path.clone(),
            SourceDiscoveryErrorKind::LengthOverflow,
        )
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity).map_err(|_| {
        SourceDiscoveryError::new(
            candidate.display_path.clone(),
            SourceDiscoveryErrorKind::Allocation {
                requested_bytes: expected_length,
            },
        )
    })?;
    let read_limit = expected_length.checked_add(1).ok_or_else(|| {
        SourceDiscoveryError::new(
            candidate.display_path.clone(),
            SourceDiscoveryErrorKind::LengthOverflow,
        )
    })?;
    file.by_ref()
        .take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            SourceDiscoveryError::io(
                DiscoveryIoOperation::ReadSource,
                candidate.display_path.clone(),
                error,
            )
        })?;
    let after_file = file.metadata().map_err(|error| {
        SourceDiscoveryError::io(
            DiscoveryIoOperation::Inspect,
            candidate.display_path.clone(),
            error,
        )
    })?;
    let after_read = std::fs::canonicalize(&candidate.physical_path).map_err(|error| {
        SourceDiscoveryError::io(
            DiscoveryIoOperation::Canonicalize,
            candidate.display_path.clone(),
            error,
        )
    })?;
    let after_path = std::fs::metadata(&after_read).map_err(|error| {
        SourceDiscoveryError::io(
            DiscoveryIoOperation::Inspect,
            candidate.display_path.clone(),
            error,
        )
    })?;
    if after_read != candidate.physical_path
        || !after_file.is_file()
        || !after_path.is_file()
        || StableFileObservation::new(&after_file) != before
        || StableFileObservation::new(&after_path) != before
        || u64::try_from(bytes.len()).ok() != Some(expected_length)
    {
        return Err(SourceDiscoveryError::new(
            candidate.display_path,
            SourceDiscoveryErrorKind::SourceChangedDuringDiscovery,
        ));
    }
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
