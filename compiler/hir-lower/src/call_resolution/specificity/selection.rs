//! The common MSC pool, before literal preferences and diagnostic ordering.

use super::DeclarationForwardingView;
use crate::Lowerer;

pub(crate) struct ApplicableDeclaration<'a> {
    pub(crate) declaration: DeclarationForwardingView<'a>,
    pub(crate) parameterized: bool,
    pub(crate) defaults: usize,
    pub(crate) vararg: bool,
}

impl Lowerer {
    pub(crate) fn most_specific_declarations(
        &self,
        candidates: &[ApplicableDeclaration<'_>],
    ) -> Vec<usize> {
        if candidates.len() < 2 {
            return (0..candidates.len()).collect();
        }
        let mut forwards = vec![vec![false; candidates.len()]; candidates.len()];
        for (source, row) in forwards.iter_mut().enumerate() {
            for (target, value) in row.iter_mut().enumerate() {
                *value = source == target
                    || self.declaration_forwards(
                        candidates[source].declaration,
                        candidates[target].declaration,
                    );
            }
        }
        let mut pool = (0..candidates.len())
            .filter(|&candidate| {
                !(0..candidates.len()).any(|other| {
                    other != candidate && forwards[other][candidate] && !forwards[candidate][other]
                })
            })
            .collect::<Vec<_>>();
        if pool.iter().any(|&index| !candidates[index].parameterized) {
            pool.retain(|&index| !candidates[index].parameterized);
        }
        let mutually_forwarding = pool.iter().all(|&source| {
            pool.iter()
                .all(|&target| forwards[source][target] && forwards[target][source])
        });
        if mutually_forwarding
            && let Some(defaults) = pool.iter().map(|&index| candidates[index].defaults).min()
        {
            pool.retain(|&index| candidates[index].defaults == defaults);
            if pool.iter().any(|&index| !candidates[index].vararg) {
                pool.retain(|&index| !candidates[index].vararg);
            }
        }
        pool
    }
}
