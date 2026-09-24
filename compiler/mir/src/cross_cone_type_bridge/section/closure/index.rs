use super::*;

pub(super) struct TargetIndex {
    rows: Vec<(MirTypeBridgeTargetV1, usize)>,
    legacy: Vec<StrongCallableDefinitionOwner>,
}
impl TargetIndex {
    pub fn build<E>(
        views: &[LocalView<'_>],
        meter: &mut BudgetMeter,
    ) -> Result<Self, MirTypeBridgeSectionError<E>> {
        let path = WirePath::root();
        let mut rows = Vec::new();
        let mut legacy = Vec::new();
        let mut providers = reserve(views.len(), meter)?;
        providers.extend(views.iter().map(|view| view.provider));
        sort_work(providers.len(), meter)?;
        providers.sort_unstable();
        meter.charge_work(providers.len() as u64, &path)?;
        if let Some(pair) = providers.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(MirTypeBridgeSectionError::DuplicateProvider(pair[0]));
        }
        for (owner, view) in views.iter().enumerate() {
            meter.try_reserve_collection_slots(&mut legacy, view.legacy.len(), &path)?;
            legacy.extend_from_slice(view.legacy);
            view.targets(|target| {
                meter.charge_work(1, &path)?;
                meter.check_table_entries(rows.len() as u64 + 1, &path)?;
                meter.try_reserve_collection_slots(&mut rows, 1, &path)?;
                rows.push((target, owner));
                Ok::<_, MirTypeBridgeSectionError<E>>(())
            })?;
        }
        sort_work(legacy.len(), meter)?;
        legacy.sort_unstable();
        meter.charge_work(legacy.len() as u64, &path)?;
        if let Some(pair) = legacy.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(MirTypeBridgeSectionError::OldCallablePartition(pair[0]));
        }
        sort_work(rows.len(), meter)?;
        rows.sort_unstable_by_key(|row| row.0);
        meter.charge_work(rows.len() as u64, &path)?;
        if let Some(pair) = rows.windows(2).find(|pair| pair[0].0 == pair[1].0) {
            return Err(MirTypeBridgeSectionError::DuplicateTarget(pair[0].0));
        }
        for (target, _) in &rows {
            if let MirTypeBridgeTargetV1::Callable(target) = target {
                search_work(legacy.len(), meter)?;
                if legacy.binary_search(target).is_ok() {
                    return Err(MirTypeBridgeSectionError::OldCallablePartition(*target));
                }
            }
        }
        Ok(Self { rows, legacy })
    }
    pub fn owner<E>(
        &self,
        target: MirTypeBridgeTargetV1,
        meter: &mut BudgetMeter,
    ) -> Result<usize, MirTypeBridgeSectionError<E>> {
        if let MirTypeBridgeTargetV1::Callable(target) = target {
            search_work(self.legacy.len(), meter)?;
            if self.legacy.binary_search(&target).is_ok() {
                return Err(MirTypeBridgeSectionError::OldCallablePartition(target));
            }
        }
        search_work(self.rows.len(), meter)?;
        self.rows
            .binary_search_by_key(&target, |row| row.0)
            .ok()
            .map(|index| self.rows[index].1)
            .ok_or(MirTypeBridgeSectionError::MissingDependency(target))
    }
}
fn search_work(count: usize, meter: &mut BudgetMeter) -> Result<(), WireError> {
    meter.charge_work(
        u64::from(usize::BITS - count.leading_zeros()) + 1,
        &WirePath::root(),
    )
}
