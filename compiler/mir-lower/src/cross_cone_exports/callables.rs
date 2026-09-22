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
    let mut retained_ordinary = 0;
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
            retained_ordinary += 1;
        } else {
            records.push(record);
        }
    }
    if retained_ordinary != ordinary.exports().len() {
        return Err(Error::IncompleteOrdinaryCallables {
            expected: ordinary.exports().len(),
            actual: retained_ordinary,
        });
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
