//! Reuse of already resolved dependency identity graphs.

use std::collections::HashSet;

use super::*;

impl PendingIdentityValidation {
    /// Imports resolved dependency entities while preserving local declarations.
    /// Shared keys are retained by reference; only merge conflicts are checked.
    pub fn register_external_graph_authorities(
        &mut self,
        graph: &ValidatedIdentityGraph,
    ) -> Result<(), IdentityValidationError> {
        self.require_registration_phase()?;
        let mut nodes = Vec::new();
        scoop_wire::allocation::try_reserve(
            &mut nodes,
            graph.candidates.len(),
            &self.resource_path,
        )
        .map_err(|error| self.poison(IdentityValidationError::Resource(error)))?;
        nodes.extend(graph.candidates.iter());
        nodes.sort_unstable_by_key(|(node, _)| **node);
        for (&node, source) in nodes {
            if let Some(candidate) = self.candidates.get(&node) {
                if candidate.layer.is_none()
                    && candidate.trusted_id.as_ref().type_id()
                        != source.trusted_id.as_ref().type_id()
                {
                    return self.fail(IdentityValidationError::IdentityCollision {
                        kind: node.kind,
                        id: node.bytes,
                    });
                }
                continue;
            }
            self.reserve_candidate_slot()?;
            self.candidates.insert(
                node,
                Candidate {
                    trusted_id: Arc::clone(&source.trusted_id),
                    layer: None,
                    resolved: true,
                    dependency_count: 0,
                    dependents: Vec::new(),
                },
            );
        }

        let mut keys = Vec::new();
        scoop_wire::allocation::try_reserve(
            &mut keys,
            graph.canonical_keys.len(),
            &self.resource_path,
        )
        .map_err(|error| self.poison(IdentityValidationError::Resource(error)))?;
        keys.extend(graph.canonical_keys.iter());
        keys.sort_unstable_by_key(|(slot, _)| (slot.kind, slot.bytes));
        let mut keyed_nodes = HashSet::new();
        scoop_wire::allocation::try_reserve_set(
            &mut keyed_nodes,
            self.canonical_keys.len(),
            &self.resource_path,
        )
        .map_err(|error| self.poison(IdentityValidationError::Resource(error)))?;
        keyed_nodes.extend(self.canonical_keys.keys().map(|slot| IdentityNode {
            kind: slot.kind,
            bytes: slot.bytes,
        }));
        for (&slot, source_key) in keys {
            let node = IdentityNode {
                kind: slot.kind,
                bytes: slot.bytes,
            };
            if self.candidates[&node].layer.is_some() {
                continue;
            }
            if let Some(existing) = self.canonical_keys.get(&slot) {
                if Arc::ptr_eq(existing, source_key) || existing.equals(source_key.as_ref()) {
                    continue;
                }
                return self.fail(IdentityValidationError::IdentityCollision {
                    kind: node.kind,
                    id: node.bytes,
                });
            }
            if keyed_nodes.contains(&node) {
                return self.fail(IdentityValidationError::IdentityCollision {
                    kind: node.kind,
                    id: node.bytes,
                });
            }
            self.reserve_canonical_key_slot()?;
            scoop_wire::allocation::try_reserve_set(&mut keyed_nodes, 1, &self.resource_path)
                .map_err(|error| self.poison(IdentityValidationError::Resource(error)))?;
            self.canonical_keys.insert(slot, Arc::clone(source_key));
            keyed_nodes.insert(node);
        }
        Ok(())
    }
}
