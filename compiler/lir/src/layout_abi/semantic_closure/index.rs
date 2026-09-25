use super::*;
use std::collections::HashMap;

pub(super) struct LayoutAbiTargetIndex {
    owners: HashMap<LayoutAbiSemanticTargetV1, usize>,
}

impl LayoutAbiTargetIndex {
    pub(super) fn build(
        views: &[&LayoutAbiExportConstituentsV1],
    ) -> Result<Self, LayoutAbiSemanticClosureError> {
        let path = WirePath::root();
        let mut owners = HashMap::new();
        for (owner, view) in views.iter().enumerate() {
            view.visit_targets(|target| {
                if owners.contains_key(&target) {
                    return Err(LayoutAbiSemanticClosureError::DuplicateTarget(target));
                }
                scoop_wire::allocation::try_reserve_map(&mut owners, 1, &path)?;
                owners.insert(target, owner);
                Ok(())
            })?;
        }
        Ok(Self { owners })
    }

    pub(super) fn owner(
        &self,
        target: LayoutAbiSemanticTargetV1,
    ) -> Result<usize, LayoutAbiSemanticClosureError> {
        self.owners
            .get(&target)
            .copied()
            .ok_or(LayoutAbiSemanticClosureError::MissingTarget(target))
    }
}
