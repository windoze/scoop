use super::*;

pub(super) struct TargetIndex {
    rows: Vec<(MirTypeBridgeTargetV1, usize)>,
    legacy: Vec<StrongCallableDefinitionOwner>,
}
impl TargetIndex {
    pub fn build<E>(views: &[LocalView<'_>]) -> Result<Self, MirTypeBridgeSectionError<E>> {
        let path = WirePath::root();
        let mut rows = Vec::new();
        let mut legacy = Vec::new();
        let mut providers = reserve(views.len())?;
        providers.extend(views.iter().map(|view| view.provider));

        providers.sort_unstable();

        if let Some(pair) = providers.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(MirTypeBridgeSectionError::DuplicateProvider(pair[0]));
        }
        for (owner, view) in views.iter().enumerate() {
            scoop_wire::allocation::try_reserve(&mut legacy, view.legacy.len(), &path)?;
            legacy.extend_from_slice(view.legacy);
            view.targets(|target| {
                scoop_wire::allocation::try_reserve(&mut rows, 1, &path)?;
                rows.push((target, owner));
                Ok::<_, MirTypeBridgeSectionError<E>>(())
            })?;
        }

        legacy.sort_unstable();

        if let Some(pair) = legacy.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(MirTypeBridgeSectionError::OldCallablePartition(pair[0]));
        }

        rows.sort_unstable_by_key(|row| row.0);

        if let Some(pair) = rows.windows(2).find(|pair| pair[0].0 == pair[1].0) {
            return Err(MirTypeBridgeSectionError::DuplicateTarget(pair[0].0));
        }
        for (target, _) in &rows {
            if let MirTypeBridgeTargetV1::Callable(target) = target {
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
    ) -> Result<usize, MirTypeBridgeSectionError<E>> {
        if let MirTypeBridgeTargetV1::Callable(target) = target {
            if self.legacy.binary_search(&target).is_ok() {
                return Err(MirTypeBridgeSectionError::OldCallablePartition(target));
            }
        }

        self.rows
            .binary_search_by_key(&target, |row| row.0)
            .ok()
            .map(|index| self.rows[index].1)
            .ok_or(MirTypeBridgeSectionError::MissingDependency(target))
    }
}
