use super::*;

pub(super) fn read_discovered_source(
    source: SelectedSource,
) -> Result<DiscoveredSource, SourceDiscoveryError> {
    let bytes = std::fs::read(source.physical_path()).map_err(|error| {
        SourceDiscoveryError::io(
            DiscoveryIoOperation::ReadSource,
            source.display_locator().as_path().to_path_buf(),
            error,
        )
    })?;
    let source_text = String::from_utf8(bytes).map_err(|_| {
        SourceDiscoveryError::new(
            source.display_locator().as_path().to_path_buf(),
            SourceDiscoveryErrorKind::InvalidUtf8Source,
        )
    })?;
    let mut discovered = DiscoveredSource::new(
        source.identity().clone(),
        source.display_locator().clone(),
        source_text,
    );
    discovered.role = source.role();
    Ok(discovered)
}
