//! Omit complete ODR objects already stored by an actual dependency.

use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{OdrGroupId, OdrMemberId};
use scoop_wire::Digest256;

use crate::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CanonicalOdrMemberDirectoryV1, LinkDefinitionOwnerV1,
    LinkIdentityClosureSectionV1, OdrMemberDirectoryEntryV1, SlibMemberId,
    UncompressedObjectMember, ValidatedCrossConeArtifactClosure,
};

type OdrBundle = Vec<(OdrGroupId, OdrMemberDirectoryEntryV1)>;

pub(super) fn select(
    objects: &[UncompressedObjectMember],
    link: &LinkIdentityClosureSectionV1,
    odr: &CanonicalOdrMemberDirectoryV1,
    dependencies: &ValidatedCrossConeArtifactClosure,
) -> BTreeSet<SlibMemberId> {
    if dependencies.artifact_count() == 0 {
        return BTreeSet::new();
    }
    let mut available = BTreeMap::<Digest256, Vec<OdrBundle>>::new();
    for defined in dependencies.dependency_symbol_owners() {
        let dependency = dependencies
            .link(defined.producer())
            .expect("dependency symbol owners belong to the same artifact closure");
        let mut bundles = bundles(defined, dependency.definition_data().odr_members());
        for record in dependency.member_records() {
            if let Some(bundle) = bundles.remove(&record.id()) {
                let candidates = available.entry(record.sha256()).or_default();
                if !candidates.contains(&bundle) {
                    candidates.push(bundle);
                }
            }
        }
    }
    let bundles = bundles(link.defined_symbols(), odr);
    objects
        .iter()
        .filter_map(|object| {
            let record = object.content();
            let bundle = bundles.get(&record.id())?;
            let candidates = available.get(&record.sha256())?;
            candidates.contains(bundle).then_some(record.id())
        })
        .collect()
}

fn bundles(
    defined: &CanonicalDefinedLinkSymbolOwnerSetV1,
    odr: &CanonicalOdrMemberDirectoryV1,
) -> BTreeMap<SlibMemberId, OdrBundle> {
    let entries = odr
        .groups()
        .iter()
        .flat_map(|group| {
            group
                .members()
                .iter()
                .map(|entry| (entry.member(), (group.group(), *entry)))
        })
        .collect::<BTreeMap<_, _>>();
    let mut objects = BTreeMap::<SlibMemberId, BTreeSet<OdrMemberId>>::new();
    let mut excluded = BTreeSet::new();
    for definition in defined.owners() {
        match definition.owner() {
            LinkDefinitionOwnerV1::OdrDefinition(member) if entries.contains_key(&member) => {
                objects
                    .entry(definition.member())
                    .or_default()
                    .insert(member);
            }
            LinkDefinitionOwnerV1::VerifierBoundary { .. } => {}
            _ => {
                excluded.insert(definition.member());
            }
        }
    }
    objects
        .into_iter()
        .filter(|(member, _)| !excluded.contains(member))
        .map(|(member, entries_in_object)| {
            let bundle = entries_in_object
                .into_iter()
                .map(|id| entries[&id])
                .collect();
            (member, bundle)
        })
        .collect()
}
