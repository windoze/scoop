use super::*;

pub(super) struct TargetIndex {
    rows: Vec<(MirTypeBridgeTargetV1, usize)>,
}
impl TargetIndex {
    pub fn build(views: &[LocalView<'_>]) -> Result<Self, MirTypeBridgeSectionError> {
        let path = WirePath::root();
        let mut rows = Vec::new();
        let mut providers = reserve(views.len())?;
        providers.extend(views.iter().map(|view| view.provider));

        providers.sort_unstable();

        if let Some(pair) = providers.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(MirTypeBridgeSectionError::DuplicateProvider(pair[0]));
        }
        for (owner, view) in views.iter().enumerate() {
            scoop_wire::allocation::try_reserve(&mut rows, view.direct.exports().len(), &path)?;
            rows.extend(view.direct.exports().iter().map(|record| {
                (
                    MirTypeBridgeTargetV1::Callable(
                        scoop_identity::CallableDefinitionOwner::Strong(record.implementation()),
                    ),
                    owner,
                )
            }));
            view.targets(|target| {
                scoop_wire::allocation::try_reserve(&mut rows, 1, &path)?;
                rows.push((target, owner));
                Ok::<_, MirTypeBridgeSectionError>(())
            })?;
        }

        rows.sort_unstable_by_key(|row| (row.0, row.1 != 0, views[row.1].provider));

        for pair in rows.windows(2).filter(|pair| pair[0].0 == pair[1].0) {
            let target = pair[0].0;
            let compatible = match (
                views[pair[0].1].record(target),
                views[pair[1].1].record(target),
            ) {
                (
                    Some(MirTypeBridgeSemanticRecordV1::Type(left)),
                    Some(MirTypeBridgeSemanticRecordV1::Type(right)),
                ) => {
                    matches!(left.origin(), MirTypeOriginV1::NominalApplication(_)) && left == right
                }
                (
                    Some(MirTypeBridgeSemanticRecordV1::Callable(left)),
                    Some(MirTypeBridgeSemanticRecordV1::Callable(right)),
                ) => {
                    matches!(
                        left.implementation(),
                        scoop_identity::CallableDefinitionOwner::Odr(_)
                    ) && left == right
                }
                (
                    Some(MirTypeBridgeSemanticRecordV1::Dispatch(left)),
                    Some(MirTypeBridgeSemanticRecordV1::Dispatch(right)),
                ) => left.is_application() && left == right,
                _ => false,
            };
            if !compatible {
                return Err(MirTypeBridgeSectionError::DuplicateTarget(target));
            }
        }
        rows.dedup_by_key(|row| row.0);
        Ok(Self { rows })
    }
    pub fn owner(&self, target: MirTypeBridgeTargetV1) -> Result<usize, MirTypeBridgeSectionError> {
        self.rows
            .binary_search_by_key(&target, |row| row.0)
            .ok()
            .map(|index| self.rows[index].1)
            .ok_or(MirTypeBridgeSectionError::MissingDependency(target))
    }
}
