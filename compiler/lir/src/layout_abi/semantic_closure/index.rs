use super::*;
pub(super) struct LayoutAbiTargetIndex {
    rows: Vec<(LayoutAbiSemanticTargetV1, ConeIdentity, usize)>,
}

impl LayoutAbiTargetIndex {
    pub(super) fn build(
        views: &[&LayoutAbiExportConstituentsV1],
    ) -> Result<Self, LayoutAbiSemanticClosureError> {
        let path = WirePath::root();
        let mut rows = Vec::new();
        for (owner, view) in views.iter().enumerate() {
            for record in view.direct_callables().exports() {
                let target = LayoutAbiSemanticTargetV1::Callable(
                    scoop_identity::CallableDefinitionOwner::Strong(record.target()),
                );
                scoop_wire::allocation::try_reserve(&mut rows, 1, &path)?;
                rows.push((target, view.provider(), owner));
            }
            view.visit_targets(|target| {
                scoop_wire::allocation::try_reserve(&mut rows, 1, &path)?;
                rows.push((target, view.provider(), owner));
                Ok::<_, LayoutAbiSemanticClosureError>(())
            })?;
        }
        rows.sort_unstable_by_key(|row| (row.0, row.1));
        for pair in rows.windows(2).filter(|pair| pair[0].0 == pair[1].0) {
            let target = pair[0].0;
            let left = odr_definition(views[pair[0].2].record(target));
            let right = odr_definition(views[pair[1].2].record(target));
            if pair[0].1 == pair[1].1 || left.is_none() || left != right {
                return Err(LayoutAbiSemanticClosureError::DuplicateTarget(target));
            }
        }
        Ok(Self { rows })
    }

    pub(super) fn owner(
        &self,
        target: LayoutAbiSemanticTargetV1,
        provider: Option<ConeIdentity>,
    ) -> Result<usize, LayoutAbiSemanticClosureError> {
        let first = self.rows.partition_point(|row| row.0 < target);
        let row = self
            .rows
            .get(first)
            .filter(|row| row.0 == target)
            .ok_or(LayoutAbiSemanticClosureError::MissingTarget(target))?;
        match provider {
            None => Ok(row.2),
            Some(expected) => self
                .rows
                .binary_search_by_key(&(target, expected), |row| (row.0, row.1))
                .map(|index| self.rows[index].2)
                .map_err(|_| LayoutAbiSemanticClosureError::Provider {
                    target: Box::new(target),
                    expected,
                    actual: row.1,
                }),
        }
    }
}

fn odr_definition(
    record: Option<LayoutAbiSemanticRecordV1<'_>>,
) -> Option<scoop_identity::ObjectDefinitionPlanId> {
    let physical = match record? {
        LayoutAbiSemanticRecordV1::Layout(record) => record.identity().physical_definition(),
        LayoutAbiSemanticRecordV1::Callable(record) => record.physical_definition(),
        LayoutAbiSemanticRecordV1::Descriptor(record) => record.physical_definition(),
        LayoutAbiSemanticRecordV1::Dispatch(record) => record.physical_definition(),
        _ => return None,
    };
    (physical.symbol().linkage() == scoop_identity::LinkageClass::OdrWeak)
        .then_some(physical.definition())
}
