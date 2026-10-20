use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::ConeIdentity;

/// Dependency counts and reverse edges for this already resolved graph.
pub(super) struct ReadyNodes {
    remaining: Vec<usize>,
    dependents: Vec<Vec<usize>>,
    ready: BTreeSet<usize>,
}

impl ReadyNodes {
    pub fn new(
        order: &[ConeIdentity],
        edges: impl Iterator<Item = (ConeIdentity, ConeIdentity)>,
    ) -> Self {
        let positions = order
            .iter()
            .enumerate()
            .map(|(position, &id)| (id, position))
            .collect::<BTreeMap<_, _>>();
        let mut remaining = vec![0; order.len()];
        let mut dependents = vec![Vec::new(); order.len()];
        for (dependent, dependency) in edges {
            let dependent = positions[&dependent];
            let dependency = positions[&dependency];
            remaining[dependent] += 1;
            dependents[dependency].push(dependent);
        }
        let ready = remaining
            .iter()
            .enumerate()
            .filter_map(|(position, &count)| (count == 0).then_some(position))
            .collect();
        Self {
            remaining,
            dependents,
            ready,
        }
    }

    pub fn pop(&mut self) -> Option<usize> {
        self.ready.pop_first()
    }

    pub fn defer(&mut self, positions: impl IntoIterator<Item = usize>) {
        self.ready.extend(positions);
    }

    pub fn complete(&mut self, position: usize) {
        for &dependent in &self.dependents[position] {
            self.remaining[dependent] -= 1;
            if self.remaining[dependent] == 0 {
                self.ready.insert(dependent);
            }
        }
    }
}
