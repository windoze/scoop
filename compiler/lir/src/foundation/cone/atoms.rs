use std::collections::HashMap;

use scoop_identity::{DefinitionAtomRole, ObjectDefinitionAtomId, ObjectDefinitionPlanId};

use super::super::DefinitionAtomRecord;
use super::DefinitionAtomResolutionError;

/// Lookup projections of one immutable canonical atom table.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct DefinitionAtomIndex {
    by_plan: HashMap<ObjectDefinitionPlanId, Vec<usize>>,
    by_role: HashMap<
        (ObjectDefinitionPlanId, DefinitionAtomRole),
        Result<ObjectDefinitionAtomId, DefinitionAtomResolutionError>,
    >,
}

impl DefinitionAtomIndex {
    pub(super) fn new(atoms: &[DefinitionAtomRecord]) -> Self {
        let mut result = Self {
            by_plan: HashMap::new(),
            by_role: HashMap::new(),
        };
        for (index, atom) in atoms.iter().enumerate() {
            let key = atom.key();
            result.by_plan.entry(key.plan()).or_default().push(index);
            result
                .by_role
                .entry((key.plan(), key.role()))
                .and_modify(|entry| *entry = Err(DefinitionAtomResolutionError::Ambiguous))
                .or_insert(Ok(atom.id()));
        }
        result
    }

    pub(super) fn for_plan(
        &self,
        plan: ObjectDefinitionPlanId,
    ) -> impl Iterator<Item = usize> + '_ {
        self.by_plan.get(&plan).into_iter().flatten().copied()
    }

    pub(super) fn resolve(
        &self,
        plan: ObjectDefinitionPlanId,
        role: DefinitionAtomRole,
    ) -> Result<ObjectDefinitionAtomId, DefinitionAtomResolutionError> {
        self.by_role
            .get(&(plan, role))
            .copied()
            .unwrap_or(Err(DefinitionAtomResolutionError::Missing))
    }
}
