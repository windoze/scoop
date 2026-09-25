use super::*;

pub(super) fn validate_all(
    bound: &BoundTypeFoundationSourcesV1<'_>,
) -> Result<(), TypeFoundationBindingError> {
    let records = bound.source.entries().definition_sources.sources();

    for source in records {
        validate(bound, source)?;
    }
    Ok(())
}

pub(super) fn validate(
    bound: &BoundTypeFoundationSourcesV1<'_>,
    source: &ExportDefinitionSourceV1,
) -> Result<(), TypeFoundationBindingError> {
    bound
        .foundation
        .validate_definition_source_location(bound.source.entries().provider, source)
        .map_err(Into::into)
}
