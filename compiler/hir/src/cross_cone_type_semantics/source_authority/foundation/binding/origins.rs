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
    use TypeFoundationBindingError as Error;
    let origin = source.origin();
    let bytes = origin.source().logical_path().as_str().len() as u64;
    let counts = bound.foundation.as_canonical().counts();
    meter.check_semantic_leaf(bytes, path)?;
    meter.charge_work(
        bytes.saturating_mul(u64::from(counts.sources.max(1).ilog2()) + 2),
        path,
    )?;
    meter.charge_work(u64::from(counts.source_contexts.max(1).ilog2()) + 1, path)?;
    if origin.source().cone() != bound.source.entries().provider {
        return Err(Error::ForeignOrigin);
    }
    let context = bound
        .foundation
        .source_context_key(origin.context())
        .ok_or(Error::MissingSourceContext(origin.context()))?;
    if context.source() != origin.source() {
        return Err(Error::SourceContextMismatch(origin.context()));
    }
    let record = bound
        .foundation
        .source_record(origin.source())
        .ok_or(Error::MissingSourceRecord)?;
    meter.charge_work(
        2 * (u64::from(record.points().len().max(1).ilog2()) + 1),
        path,
    )?;
    record
        .require_points([origin.span().start_byte(), origin.span().end_byte()])
        .map_err(|error| Error::MissingSourcePoint(error.byte_offset))
}
