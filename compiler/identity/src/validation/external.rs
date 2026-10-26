//! Merge already resolved dependency nodes without rebuilding lookup indexes.

use super::*;

impl PendingIdentityValidation {
    /// Imports resolved dependency entities while preserving local declarations.
    /// Canonical keys remain shared; only merge conflicts require comparison.
    pub fn register_external_graph_authorities(
        &mut self,
        graph: &ValidatedIdentityGraph,
    ) -> Result<(), IdentityValidationError> {
        self.require_registration_phase()?;
        let mut conflict = None;
        for (&node, source) in graph.candidates.iter() {
            if let Some(candidate) = self.candidates.get_mut(&node) {
                if candidate.layer.is_some() {
                    continue;
                }
                let matches = candidate.trusted_id.as_ref().type_id()
                    == source.trusted_id.as_ref().type_id()
                    && match (&candidate.canonical_key, &source.canonical_key) {
                        (Some(existing), Some(key)) => {
                            Arc::ptr_eq(existing, key) || existing.equals(key.as_ref())
                        }
                        _ => true,
                    };
                if !matches {
                    conflict =
                        Some(conflict.map_or(node, |previous: IdentityNode| previous.min(node)));
                } else if candidate.canonical_key.is_none() {
                    candidate.canonical_key.clone_from(&source.canonical_key);
                }
            } else {
                self.reserve_candidate_slot()?;
                let mut candidate = source.clone();
                candidate.layer = None;
                self.candidates.insert(node, candidate);
            }
        }
        if let Some(node) = conflict {
            return self.fail(IdentityValidationError::IdentityCollision {
                kind: node.kind,
                id: node.bytes,
            });
        }
        Ok(())
    }
}
