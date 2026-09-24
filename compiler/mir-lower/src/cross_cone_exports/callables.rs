use super::*;
use scoop_identity::DependencyCallableDeclarationId;

pub(super) fn combine<const N: usize>(
    tables: [mir::CanonicalMirCallableBindingsV1; N],
    ordinary: &mir::CrossConeMirBridgeSectionV1,
    meter: &mut BudgetMeter,
) -> Result<mir::CanonicalMirCallableBindingsV1, Error> {
    let count = tables.iter().try_fold(0usize, |count, table| {
        count
            .checked_add(table.entries().len())
            .ok_or(Error::CountOverflow)
    })?;
    let mut records = reserve(count, meter)?;
    for table in tables {
        records.extend(table.into_entries());
    }
    meter.charge_work(
        (count as u64).saturating_mul(u64::from(count.checked_ilog2().unwrap_or(0)) + 1),
        &WirePath::root(),
    )?;
    // Detect duplicate implementations before removing the old partition.
    let combined =
        mir::CanonicalMirCallableBindingsV1::try_new(records).map_err(Error::Callables)?;
    let mut records = reserve(count, meter)?;
    for record in combined.into_entries() {
        meter.charge_work(
            u64::from(ordinary.exports().len().checked_ilog2().unwrap_or(0)) + 1,
            &WirePath::root(),
        )?;
        let declaration = match record.origin() {
            mir::MirCallableOriginV1::Function(id) => {
                Some(DependencyCallableDeclarationId::Function(*id))
            }
            mir::MirCallableOriginV1::Accessor(id) => {
                Some(DependencyCallableDeclarationId::PropertyAccessor(*id))
            }
            mir::MirCallableOriginV1::Constructor(_)
            | mir::MirCallableOriginV1::Generated { .. } => None,
        };
        if let Some(old) = declaration.and_then(|declaration| ordinary.export(declaration)) {
            meter.charge_work(
                scoop_wire::encoded_length(&record).map_err(Error::Encoding)?,
                &WirePath::root(),
            )?;
            if old.implementation() != record.implementation()
                || old.signature() != record.semantic_signature().exact()
                || old.signature() != record.lowered_signature().exact()
            {
                return Err(Error::OrdinaryCallableMismatch(record.implementation()));
            }
        } else {
            records.push(record);
        }
    }
    // Filtering preserves the canonical order, but the public constructor
    // deliberately checks it again at the final table boundary.
    meter.charge_work(
        (records.len() as u64)
            .saturating_mul(u64::from(records.len().checked_ilog2().unwrap_or(0)) + 1),
        &WirePath::root(),
    )?;
    mir::CanonicalMirCallableBindingsV1::try_new(records).map_err(Error::Callables)
}

pub(super) fn validate_ordinary(
    input: MirTypeBridgeExportInputV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let expected = hir::select_ordinary_source_callables(
        input.mir.module().cone,
        input.public,
        input.nominal_classifier,
        input.identities,
        meter,
    )
    .map_err(|error| match error {
        hir::SharedTypeMetadataError::Resource(error) => Error::Resource(error),
        error => Error::OrdinarySource(Box::new(error)),
    })?;
    if expected.len() != input.ordinary.exports().len() {
        return Err(Error::IncompleteOrdinaryCallables {
            expected: expected.len(),
            actual: input.ordinary.exports().len(),
        });
    }
    for (declaration, source) in expected {
        meter.charge_work(
            1 + u64::from(input.ordinary.exports().len().max(1).ilog2()),
            &WirePath::root(),
        )?;
        let classified = input
            .nominal_classifier
            .classify_callable_metered(source, meter, &WirePath::root())
            .map_err(Error::OrdinaryClassification)?
            .ok_or(Error::OrdinaryCallableMismatch(
                declaration.implementation(),
            ))?;
        let actual = input
            .ordinary
            .export(declaration)
            .ok_or(Error::OrdinaryCallableMismatch(
                declaration.implementation(),
            ))?;
        if actual.implementation() != declaration.implementation()
            || actual.signature() != classified.signature()
        {
            return Err(Error::OrdinaryCallableMismatch(
                declaration.implementation(),
            ));
        }
    }
    Ok(())
}
