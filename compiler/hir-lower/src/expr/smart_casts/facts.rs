use super::*;
use std::collections::BTreeSet;

pub(super) type TypeFacts = BTreeMap<hir::BindingId, Vec<TypeId>>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SmartCastFacts {
    pub(crate) types: TypeFacts,
    pub(crate) aliases: BTreeSet<(hir::BindingId, hir::BindingId)>,
    pub(crate) booleans: BTreeMap<hir::BindingId, BooleanFacts>,
    pub(crate) escaped: BTreeSet<hir::BindingId>,
}

/// Implications of an evaluated Boolean value. None denotes an impossible result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BooleanFacts {
    pub(crate) yes: Option<TypeFacts>,
    pub(crate) no: Option<TypeFacts>,
}

impl SmartCastFacts {
    pub(crate) fn get(&self, binding: &hir::BindingId) -> Option<&Vec<TypeId>> {
        self.types.get(binding)
    }

    pub(crate) fn insert(&mut self, binding: hir::BindingId, types: Vec<TypeId>) {
        self.types.insert(binding, types);
    }

    pub(crate) fn related(&self, binding: hir::BindingId) -> Vec<hir::BindingId> {
        std::iter::once(binding)
            .chain(self.aliases.iter().filter_map(|&(left, right)| {
                if left == binding {
                    Some(right)
                } else if right == binding {
                    Some(left)
                } else {
                    None
                }
            }))
            .collect()
    }

    pub(crate) fn copy_binding(&mut self, target: hir::BindingId, source: hir::BindingId) {
        if target == source || self.escaped.contains(&source) || self.escaped.contains(&target) {
            return;
        }
        let mut related: BTreeSet<_> = self.related(source).into_iter().collect();
        related.insert(target);
        for &left in &related {
            for &right in
                related.range((std::ops::Bound::Excluded(left), std::ops::Bound::Unbounded))
            {
                self.aliases.insert((left, right));
            }
        }
        if let Some(types) = self.types.get(&source).cloned() {
            self.types.insert(target, types);
        }
    }

    pub(crate) fn forget(&mut self, binding: hir::BindingId) {
        self.types.remove(&binding);
        self.aliases
            .retain(|&(left, right)| left != binding && right != binding);
        self.booleans.remove(&binding);
        for facts in self.booleans.values_mut() {
            for path in [&mut facts.yes, &mut facts.no].into_iter().flatten() {
                path.remove(&binding);
            }
        }
    }

    pub(crate) fn expose_address(&mut self, binding: hir::BindingId) {
        self.forget(binding);
        self.escaped.insert(binding);
    }
}
