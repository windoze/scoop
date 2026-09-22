use super::*;

pub(super) struct Inventory {
    pub types: Vec<PersistentExactTypeId>,
    pub callables: Vec<StrongCallableDefinitionOwner>,
    pub dispatch: Vec<PersistentExactTypeId>,
    pub objects: Vec<PersistentObjectValueId>,
    pub roots: Vec<PersistentTypeId>,
}
impl Inventory {
    pub fn from_source(
        input: MirTypeBridgeExportInputV1<'_>,
        source: &mir::MirTypeBridgeExportConstituentsV1,
        meter: &mut BudgetMeter,
    ) -> Result<Self, MirTypeBridgeSourceProjectionError> {
        let mut roots = collect(
            input
                .hir
                .output()
                .local
                .materialization()
                .roots()
                .iter()
                .map(|r| r.source()),
            meter,
        )?;
        meter.charge_work(
            (roots.len() as u64)
                .saturating_mul(u64::from(roots.len().checked_ilog2().unwrap_or(0)) + 1),
            &WirePath::root(),
        )?;
        roots.sort_unstable();
        source
            .shapes()
            .validate_required_sources(&roots, meter)
            .map_err(MirTypeBridgeSourceProjectionError::Shapes)?;
        Ok(Self {
            types: collect(source.types().records().iter().map(|r| r.exact()), meter)?,
            callables: collect(
                source
                    .callables()
                    .entries()
                    .iter()
                    .map(|r| r.implementation()),
                meter,
            )?,
            dispatch: collect(source.dispatch().records().iter().map(|r| r.owner()), meter)?,
            objects: collect(source.objects().records().iter().map(|r| r.value()), meter)?,
            roots,
        })
    }
}

fn collect<T>(
    source: impl ExactSizeIterator<Item = T>,
    meter: &mut BudgetMeter,
) -> Result<Vec<T>, MirTypeBridgeSourceProjectionError> {
    let count = source.len();
    let path = WirePath::root();
    meter.check_table_entries(count as u64, &path)?;
    meter.charge_work(count as u64, &path)?;
    meter.charge_owned_bytes(
        (count as u64).saturating_mul(std::mem::size_of::<T>() as u64),
        &path,
    )?;
    let mut values = Vec::new();
    meter.try_reserve_collection_slots(&mut values, count, &path)?;
    values.extend(source);
    Ok(values)
}
