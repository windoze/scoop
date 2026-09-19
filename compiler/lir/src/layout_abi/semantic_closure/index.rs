use super::*;
use std::collections::HashMap;

pub(super) struct LayoutAbiTargetIndex {
    owners: HashMap<LayoutAbiSemanticTargetV1, usize>,
}

impl LayoutAbiTargetIndex {
    pub(super) fn build(
        views: &[&LayoutAbiExportConstituentsV1],
        meter: &mut BudgetMeter,
    ) -> Result<Self, LayoutAbiSemanticClosureError> {
        let path = WirePath::root();
        let mut owners = HashMap::new();
        for (owner, view) in views.iter().enumerate() {
            view.visit_targets(|target| {
                meter.charge_work(1, &path)?;
                if owners.contains_key(&target) {
                    return Err(LayoutAbiSemanticClosureError::DuplicateTarget(target));
                }
                meter.try_reserve_map_slots(&mut owners, 1, &path)?;
                owners.insert(target, owner);
                Ok(())
            })?;
        }
        Ok(Self { owners })
    }

    pub(super) fn owner(
        &self,
        target: LayoutAbiSemanticTargetV1,
        meter: &mut BudgetMeter,
    ) -> Result<usize, LayoutAbiSemanticClosureError> {
        meter.charge_work(1, &WirePath::root())?;
        self.owners
            .get(&target)
            .copied()
            .ok_or(LayoutAbiSemanticClosureError::MissingTarget(target))
    }
}
