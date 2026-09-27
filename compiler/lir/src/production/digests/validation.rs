use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{
    CborIdentityRecord, DigestKind, DigestNodeId, DigestOwnerAndRoleKey, DigestPatchIntentId,
    DigestPatchIntentKey, DigestSemanticFieldRole, ObjectDefinitionPlanId,
    ObjectDefinitionPlanRole, StrongDefinitionRole,
};

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
    OdrDefinition(DigestNodeId),
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
    ForeignPatchSource {
        node: DigestNodeId,
        source: DigestNodeId,
    },
    PatchSourceKind {
        node: DigestNodeId,
        kind: DigestKind,
        role: DigestSemanticFieldRole,
    },
    DuplicatePatchIntent {
        node: DigestNodeId,
        intent: DigestPatchIntentId,
    },
    DuplicatePatchWriter {
        first: DigestPatchIntentId,
        second: DigestPatchIntentId,
    },
    MissingPatchTarget(DigestPatchIntentId),
    AmbiguousPatchTarget(DigestPatchIntentId),
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
    let mut writers = BTreeMap::new();

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
        if let Some(intent) = first_duplicate_patch(&node.patch_intents) {
            return Err(DigestPlanError::DuplicatePatchIntent {
                node: node.id(),
                intent,
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

        for patch in &node.patch_intents {
            let key = patch.key();
            if key.source() != node.id() {
                return Err(DigestPlanError::ForeignPatchSource {
                    node: node.id(),
                    source: key.source(),
                });
            }
            if !key.semantic_field_role().accepts_source(node.kind()) {
                return Err(DigestPlanError::PatchSourceKind {
                    node: node.id(),
                    kind: node.kind(),
                    role: key.semantic_field_role(),
                });
            }
            validate_patch_target(patch.id(), key, foundation)?;
            let writer_key = (
                key.target_definition(),
                key.atom_role(),
                key.semantic_field_role(),
            );
            if let Some(first) = writers.insert(writer_key, patch.id()) {
                return Err(DigestPlanError::DuplicatePatchWriter {
                    first,
                    second: patch.id(),
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
        // may belong to a dependency; only physical patch targets must be local.
        DigestOwnerAndRoleKey::Layout(_) | DigestOwnerAndRoleKey::Scan(_) => true,
        DigestOwnerAndRoleKey::LirDefinition(id)
        | DigestOwnerAndRoleKey::ObjectSupport(id)
        | DigestOwnerAndRoleKey::ObjectDefinition(id) => foundation
            .definition_atoms()
            .iter()
            .any(|record| record.id() == id),
        DigestOwnerAndRoleKey::StackmapRecord(id) => foundation.contains_safepoint_site(id),
        DigestOwnerAndRoleKey::OdrMemberDefinition(_) => {
            return Err(DigestPlanError::OdrDefinition(node.id()));
        }
        DigestOwnerAndRoleKey::StrongRegistration(plan) => {
            let Some(record) = foundation
                .definition_plans()
                .iter()
                .find(|record| record.id() == plan)
            else {
                return Err(DigestPlanError::UnknownOwner(node.id()));
            };
            if !matches!(
                record.key().definition_role(),
                ObjectDefinitionPlanRole::Strong(
                    StrongDefinitionRole::RootRegistration
                        | StrongDefinitionRole::ImmortalRegistration
                        | StrongDefinitionRole::InitializationRegistration
                        | StrongDefinitionRole::TypeRegistration
                        | StrongDefinitionRole::SafepointRegistration
                        | StrongDefinitionRole::CallableRegistration
                )
            ) {
                return Err(DigestPlanError::NonRegistrationPlan {
                    node: node.id(),
                    plan,
                });
            }
            true
        }
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

fn validate_patch_target(
    intent: DigestPatchIntentId,
    key: &DigestPatchIntentKey,
    foundation: &crate::ConeLirFoundation,
) -> Result<(), DigestPlanError> {
    let count = foundation
        .definition_atoms()
        .iter()
        .filter(|record| {
            record.key().plan() == key.target_definition() && record.key().role() == key.atom_role()
        })
        .count();
    match count {
        0 => Err(DigestPlanError::MissingPatchTarget(intent)),
        1 => Ok(()),
        _ => Err(DigestPlanError::AmbiguousPatchTarget(intent)),
    }
}

fn allows_input(node: DigestKind, input: DigestKind) -> bool {
    use DigestKind as K;
    match node {
        K::SourceSignature | K::Layout | K::Scan | K::LirDefinition | K::ObjectSupport => false,
        K::StackmapRecord => matches!(input, K::SourceSignature | K::ObjectSupport),
        K::ObjectDefinition => matches!(
            input,
            K::SourceSignature
                | K::Layout
                | K::Scan
                | K::LirDefinition
                | K::ObjectSupport
                | K::StackmapRecord
        ),
        K::OdrDefinition => matches!(
            input,
            K::LirDefinition | K::ObjectDefinition | K::StackmapRecord
        ),
        K::StrongRegistration => matches!(
            input,
            K::SourceSignature
                | K::Layout
                | K::Scan
                | K::LirDefinition
                | K::ObjectDefinition
                | K::StackmapRecord
        ),
        K::RuntimeImage => matches!(
            input,
            K::SourceSignature
                | K::Layout
                | K::Scan
                | K::ObjectDefinition
                | K::StackmapRecord
                | K::StrongRegistration
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

pub(super) fn first_duplicate_patch(
    patches: &[CborIdentityRecord<DigestPatchIntentId, DigestPatchIntentKey>],
) -> Option<DigestPatchIntentId> {
    for pair in patches.windows(2) {
        if pair[0].id() == pair[1].id() {
            return Some(pair[0].id());
        }
    }
    None
}
