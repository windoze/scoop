use super::{source::Source, *};
use crate::{
    NestedSourceSupportV1, ProtectedDeclarationInterfaceV1, ProtectedNestedSourceInterfaceV1,
};

pub(super) fn sources<'a, E>(
    protected: CheckedProtectedDeclarationSourcesV1<'a>,
    inheritance: CheckedNominalInheritanceInterfacesV1<'a>,
    meter: &mut BudgetMeter,
) -> Result<Vec<Source<'a>>, ProtectedSourceClosureError<E>> {
    use ProtectedSourceClosureError as Error;
    let path = WirePath::root();
    let mut records = Vec::new();
    let mut pending: Vec<(&ProtectedNestedSourceInterfaceV1, u64)> = Vec::new();
    reserve(&mut records, protected.table().records().len(), meter)?;
    meter
        .try_reserve_collection_slots(&mut pending, protected.table().records().len(), &path)
        .map_err(Error::Resource)?;
    for record in protected.table().records() {
        meter.charge_nodes(1, &path).map_err(Error::Resource)?;
        match record {
            ProtectedDeclarationInterfaceV1::Callable(record) => {
                add(&mut records, Source::ProtectedCallable(record))
            }
            ProtectedDeclarationInterfaceV1::Constructor(record) => {
                add(&mut records, Source::ProtectedConstructor(record))
            }
            ProtectedDeclarationInterfaceV1::Property(_) => {}
            ProtectedDeclarationInterfaceV1::NestedNominal(record) => {
                pending.push((record.payload().source_interface(), 1))
            }
        }
    }
    for owner in inheritance.table().records() {
        meter.charge_nodes(1, &path).map_err(Error::Resource)?;
        reserve(&mut records, owner.constructors().records().len(), meter)?;
        for constructor in owner.constructors().records() {
            meter.charge_nodes(1, &path).map_err(Error::Resource)?;
            records.push(Source::SupportConstructor(constructor.source()));
        }
    }
    while let Some((source, depth)) = pending.pop() {
        meter
            .check_semantic_depth(depth, &path)
            .map_err(Error::Resource)?;
        meter.charge_nodes(1, &path).map_err(Error::Resource)?;
        let support = source.source_support().records();
        reserve(&mut records, support.len(), meter)?;
        meter
            .try_reserve_collection_slots(&mut pending, support.len(), &path)
            .map_err(Error::Resource)?;
        for entry in support {
            meter.charge_nodes(1, &path).map_err(Error::Resource)?;
            match entry {
                NestedSourceSupportV1::Callable(record) => {
                    add(&mut records, Source::SupportCallable(record))
                }
                NestedSourceSupportV1::Constructor(record) => {
                    add(&mut records, Source::SupportConstructor(record))
                }
                NestedSourceSupportV1::Property(_) => {}
                NestedSourceSupportV1::NestedNominal(record) => {
                    let next = depth.checked_add(1).ok_or_else(|| {
                        Error::Resource(scoop_wire::WireError::new(
                            scoop_wire::WireErrorKind::IntegerOutOfRange,
                            path.clone(),
                            None,
                        ))
                    })?;
                    meter
                        .check_semantic_depth(next, &path)
                        .map_err(Error::Resource)?;
                    pending.push((record.payload().source_interface(), next));
                }
            }
        }
    }
    let size = records.len() as u64;
    let comparisons = size
        .saturating_mul(u64::from(u64::BITS - size.leading_zeros()))
        .saturating_mul(64);
    meter
        .charge_work(comparisons, &path)
        .map_err(Error::Resource)?;
    records.sort_unstable_by_key(Source::owner);
    for pair in records.windows(2) {
        meter.charge_work(64, &path).map_err(Error::Resource)?;
        if pair[0].owner() != pair[1].owner() {
            continue;
        }
        let bytes = [pair[0].payload(), pair[1].payload()]
            .into_iter()
            .try_fold(0_u64, |sum, payload| {
                scoop_wire::encoded_length(payload).map(|size| sum.saturating_add(size))
            })
            .map_err(Error::Encoding)?;
        let access = [pair[0].access(), pair[1].access()]
            .into_iter()
            .try_fold(0_u64, |sum, source| {
                scoop_wire::encoded_length(source).map(|size| sum.saturating_add(size))
            })
            .map_err(Error::Encoding)?;
        meter
            .charge_work(bytes.saturating_add(access), &path)
            .map_err(Error::Resource)?;
        if pair[0].payload() != pair[1].payload() || pair[0].access() != pair[1].access() {
            return Err(Error::ConflictingSource(pair[0].owner()));
        }
    }
    records.dedup_by_key(|record| record.owner());
    Ok(records)
}
fn add<'a>(records: &mut Vec<Source<'a>>, record: Source<'a>) {
    if !matches!(record.owner(), CallableTemplateOrigin::Accessor(_)) {
        records.push(record);
    }
}
fn reserve<E>(
    records: &mut Vec<Source<'_>>,
    count: usize,
    meter: &mut BudgetMeter,
) -> Result<(), ProtectedSourceClosureError<E>> {
    let path = WirePath::root();
    meter
        .check_table_entries((records.len() as u64).saturating_add(count as u64), &path)
        .map_err(ProtectedSourceClosureError::Resource)?;
    meter
        .try_reserve_collection_slots(records, count, &path)
        .map_err(ProtectedSourceClosureError::Resource)
}
