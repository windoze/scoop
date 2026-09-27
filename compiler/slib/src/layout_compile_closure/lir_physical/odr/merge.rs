use std::collections::btree_map::Entry;

use super::*;
use crate::layout_compile_closure::lir_physical::PhysicalImportsReplayedCrossConeLayoutSections;
use crate::{LinkDefinitionOwnerV1, ReplayedLayoutLinkSymbolUsesV1};

/// Compare physical definitions from artifacts whose own semantics, objects,
/// and imports have already been checked. This does not read or rehash them.
pub fn merge_cross_cone_odr_definitions<'a>(
    artifacts: impl IntoIterator<
        Item = (
            &'a PhysicalImportsReplayedCrossConeLayoutSections,
            &'a ReplayedLayoutLinkSymbolUsesV1,
        ),
    >,
) -> Result<MergedOdrDefinitions, OdrDefinitionMergeError> {
    let mut members = BTreeMap::<_, MergedOdrMemberDefinition>::new();
    let mut groups = BTreeMap::<OdrGroupId, (Arc<SpecializationKey>, ConeIdentity)>::new();
    let mut keys = BTreeMap::<OdrMemberId, (Arc<OdrMemberKey>, ConeIdentity)>::new();
    let mut symbol_owners = BTreeMap::new();
    let mut previous_artifacts = BTreeMap::new();

    for (sections, artifact) in artifacts {
        let provider = artifact.provider();
        if previous_artifacts.insert(provider, artifact).is_some() {
            return Err(OdrDefinitionMergeError::DuplicateArtifact(provider));
        }
        let mut primary = BTreeMap::new();
        for definition in artifact.defined_symbols().owners() {
            match symbol_owners.entry(definition.symbol()) {
                Entry::Vacant(slot) => {
                    slot.insert((provider, definition.owner()));
                }
                Entry::Occupied(slot) => {
                    let (first, owner) = *slot.get();
                    if owner != definition.owner() {
                        return Err(OdrDefinitionMergeError::SymbolOwnerConflict {
                            first,
                            second: provider,
                            symbol: definition.symbol().to_vec(),
                        });
                    }
                }
            }
            if let LinkDefinitionOwnerV1::OdrDefinition(member) = definition.owner() {
                primary.insert(member, definition);
            }
        }
        for group in artifact.production_projection().odr_members().groups() {
            let group_key = sections
                .identity_graph()
                .canonical_key::<_, SpecializationKey>(group.group())
                .map_err(|source| OdrDefinitionMergeError::Identity { provider, source })?;
            let group_conflict = match groups.entry(group.group()) {
                Entry::Vacant(slot) => {
                    slot.insert((Arc::clone(&group_key), provider));
                    None
                }
                Entry::Occupied(slot) => {
                    let (previous, first) = slot.get();
                    (previous != &group_key).then_some(*first)
                }
            };
            for &entry in group.members() {
                let conflict = |first, difference| {
                    OdrDefinitionMergeError::Conflict(Box::new(OdrMemberConflict {
                        first,
                        second: provider,
                        group: group.group(),
                        member: entry.member(),
                        role: entry.role(),
                        difference,
                    }))
                };
                if let Some(first) = group_conflict {
                    return Err(conflict(first, OdrDefinitionDifference::GroupKey));
                }
                let key = sections
                    .identity_graph()
                    .canonical_key::<_, OdrMemberKey>(entry.member())
                    .map_err(|source| OdrDefinitionMergeError::Identity { provider, source })?;
                match keys.entry(entry.member()) {
                    Entry::Vacant(slot) => {
                        slot.insert((Arc::clone(&key), provider));
                    }
                    Entry::Occupied(slot) => {
                        let (previous, first) = slot.get();
                        if previous != &key {
                            return Err(conflict(*first, OdrDefinitionDifference::MemberKey));
                        }
                    }
                }
                let definition = primary.get(&entry.member()).ok_or(
                    OdrDefinitionMergeError::MissingPhysicalDefinition {
                        provider,
                        member: entry.member(),
                    },
                )?;
                let candidate = OdrDefinitionCandidate {
                    provider,
                    definition: (*definition).clone(),
                };
                match members.entry((group.group(), entry.member())) {
                    Entry::Vacant(slot) => {
                        slot.insert(MergedOdrMemberDefinition {
                            group_key: Arc::clone(&group_key),
                            key,
                            entry,
                            first: candidate,
                            additional: Vec::new(),
                        });
                    }
                    Entry::Occupied(mut slot) => {
                        let previous = slot.get_mut();
                        let first = previous.first.provider;
                        if previous.entry.abi() != entry.abi() {
                            return Err(conflict(first, OdrDefinitionDifference::Abi));
                        }
                        if previous.entry.definition() != entry.definition() {
                            let original = previous_artifacts[&first];
                            let difference =
                                content::difference(original, artifact, entry.member())?;
                            return Err(conflict(first, difference));
                        }
                        previous.additional.push(candidate);
                    }
                }
            }
        }
    }
    Ok(MergedOdrDefinitions { members })
}
