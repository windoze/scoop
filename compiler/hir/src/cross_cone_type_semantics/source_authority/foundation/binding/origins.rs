use super::*;

pub(super) fn validate_all(
    bound: &BoundTypeFoundationSourcesV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), TypeFoundationBindingError> {
    let records = bound.source.entries().definition_sources.sources();
    let path = WirePath::root().field(7);
    meter.check_table_entries(records.len() as u64, &path)?;
    for (index, source) in records.iter().enumerate() {
        let path = path.clone().index(index as u64);
        validate(bound, source, meter, &path)?;
    }
    Ok(())
}

pub(super) fn validate(
    bound: &BoundTypeFoundationSourcesV1<'_>,
    source: &ExportDefinitionSourceV1,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeFoundationBindingError> {
    bound
        .foundation
        .validate_definition_source_location(bound.source.entries().provider, source, meter, path)
        .map_err(Into::into)
}
