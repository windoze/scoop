use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{DigestKind, DigestNodeId, DigestOwnerAndRoleKey, ObjectDefinitionPlanId};

use super::{DigestInputRefV1, DigestNodeV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DigestPlanError {
    DuplicateNode(DigestNodeId),
    MissingRuntimeImage,
    MultipleRuntimeImages {
        first: DigestNodeId,
        second: DigestNodeId,
    },
    ForeignRuntimeImage {
        node: DigestNodeId,
        expected: scoop_identity::ConeIdentity,
        actual: scoop_identity::ConeIdentity,
    },
    UnknownOwner(DigestNodeId),
    NonRegistrationPlan {
        node: DigestNodeId,
        plan: ObjectDefinitionPlanId,
    },
    MissingInput {
        node: DigestNodeId,
        input: DigestNodeId,
    },
    InputKindMismatch {
        node: DigestNodeId,
        input: DigestNodeId,
        declared: DigestKind,
        actual: DigestKind,
    },
    DisallowedEdge {
        node: DigestNodeId,
        kind: DigestKind,
        input: DigestNodeId,
        input_kind: DigestKind,
    },
    Cycle,
    DuplicateInput {
        node: DigestNodeId,
        input: DigestNodeId,
    },
}

pub(super) fn validate_node_order(nodes: &[DigestNodeV1]) -> Result<(), DigestPlanError> {
    let mut ids = BTreeSet::new();
    for node in nodes {
        if !ids.insert(node.id()) {
            return Err(DigestPlanError::DuplicateNode(node.id()));
        }
    }
    Ok(())
}

pub(super) fn validate_plan(
    nodes: &[DigestNodeV1],
    foundation: &crate::ConeLirFoundation,
) -> Result<(), DigestPlanError> {
    validate_node_order(nodes)?;
    let known = nodes
        .iter()
        .map(|node| (node.id(), node.kind()))
        .collect::<BTreeMap<_, _>>();
    let mut image = None;
    for node in nodes {
        validate_owner(node, foundation)?;
        if node.kind() == DigestKind::RuntimeImage {
            if let Some(first) = image.replace(node.id()) {
                return Err(DigestPlanError::MultipleRuntimeImages {
                    first,
                    second: node.id(),
                });
            }
        }
        if let Some(input) = first_duplicate_input(&node.direct_inputs) {
            return Err(DigestPlanError::DuplicateInput {
                node: node.id(),
                input,
            });
        }

        for input in &node.direct_inputs {
            let Some(actual_kind) = known.get(&input.node()).copied() else {
                return Err(DigestPlanError::MissingInput {
                    node: node.id(),
                    input: input.node(),
                });
            };
            if input.kind() != actual_kind {
                return Err(DigestPlanError::InputKindMismatch {
                    node: node.id(),
                    input: input.node(),
                    declared: input.kind(),
                    actual: actual_kind,
                });
            }
            if !allows_input(node.kind(), actual_kind) {
                return Err(DigestPlanError::DisallowedEdge {
                    node: node.id(),
                    kind: node.kind(),
                    input: input.node(),
                    input_kind: actual_kind,
                });
            }
        }
    }

    if image.is_none() {
        return Err(DigestPlanError::MissingRuntimeImage);
    }
    validate_acyclic(nodes, &known)
}

fn validate_owner(
    node: &DigestNodeV1,
    foundation: &crate::ConeLirFoundation,
) -> Result<(), DigestPlanError> {
    let known = match node.key().owner_and_role() {
        DigestOwnerAndRoleKey::SourceSignature(id) => foundation.contains_callable_body(id),
        // Layout and scan ids are resolved typed references. Their definitions
        // may belong to a dependency.
        DigestOwnerAndRoleKey::Layout(_) | DigestOwnerAndRoleKey::Scan(_) => true,
        DigestOwnerAndRoleKey::ObjectSupport(id) | DigestOwnerAndRoleKey::ObjectDefinition(id) => {
            foundation.definition_atom(id).is_some()
        }
        DigestOwnerAndRoleKey::StackmapRecord(id) => foundation.contains_safepoint_site(id),
        DigestOwnerAndRoleKey::RuntimeImage(actual) => {
            if actual != foundation.producer() {
                return Err(DigestPlanError::ForeignRuntimeImage {
                    node: node.id(),
                    expected: foundation.producer(),
                    actual,
                });
            }
            true
        }
    };
    if known {
        Ok(())
    } else {
        Err(DigestPlanError::UnknownOwner(node.id()))
    }
}

fn allows_input(node: DigestKind, input: DigestKind) -> bool {
    use DigestKind as K;
    match node {
        K::SourceSignature | K::Layout | K::Scan | K::ObjectSupport => false,
        K::StackmapRecord => matches!(input, K::SourceSignature | K::ObjectSupport),
        K::ObjectDefinition => matches!(
            input,
            K::ObjectDefinition
                | K::SourceSignature
                | K::Layout
                | K::Scan
                | K::ObjectSupport
                | K::StackmapRecord
        ),
        K::RuntimeImage => matches!(
            input,
            K::SourceSignature | K::Layout | K::Scan | K::ObjectDefinition | K::StackmapRecord
        ),
    }
}

fn validate_acyclic(
    nodes: &[DigestNodeV1],
    known: &BTreeMap<DigestNodeId, DigestKind>,
) -> Result<(), DigestPlanError> {
    let mut indegree = nodes
        .iter()
        .map(|node| (node.id(), node.direct_inputs.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<DigestNodeId, Vec<DigestNodeId>>::new();
    for node in nodes {
        for input in &node.direct_inputs {
            if known.contains_key(&input.node()) {
                dependents.entry(input.node()).or_default().push(node.id());
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(id, count)| (*count == 0).then_some(*id))
        .collect::<BTreeSet<_>>();
    let mut visited = 0_usize;
    while let Some(id) = ready.pop_first() {
        visited += 1;
        for dependent in dependents.get(&id).into_iter().flatten() {
            let Some(count) = indegree.get_mut(dependent) else {
                return Err(DigestPlanError::MissingInput {
                    node: *dependent,
                    input: id,
                });
            };
            *count -= 1;
            if *count == 0 {
                ready.insert(*dependent);
            }
        }
    }
    if visited == nodes.len() {
        Ok(())
    } else {
        Err(DigestPlanError::Cycle)
    }
}

pub(super) fn first_duplicate_input(inputs: &[DigestInputRefV1]) -> Option<DigestNodeId> {
    for pair in inputs.windows(2) {
        if pair[0].sort_key() == pair[1].sort_key() {
            return Some(pair[0].node());
        }
    }
    None
}
