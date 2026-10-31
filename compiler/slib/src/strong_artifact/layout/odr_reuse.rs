//! Omit complete ODR objects already stored by an actual dependency.

use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{ObjectDefinitionPlanId, OdrGroupId, OdrMemberId};
use scoop_lir::LayoutAbiExportConstituentsV1;
use scoop_wire::Digest256;

use crate::{
    CanonicalDefinedLinkSymbolOwnerSetV1, CanonicalOdrMemberDirectoryV1, LinkDefinitionOwnerV1,
    LinkIdentityClosureSectionV1, OdrMemberDirectoryEntryV1, SlibMember, SlibMemberId,
    ValidatedCrossConeArtifactClosure,
};

type OdrBundle = Vec<(OdrGroupId, OdrMemberDirectoryEntryV1)>;

pub(super) fn select(
    objects: &[SlibMember],
    link: &LinkIdentityClosureSectionV1,
    odr: &CanonicalOdrMemberDirectoryV1,
    exports: &LayoutAbiExportConstituentsV1,
    dependencies: &ValidatedCrossConeArtifactClosure,
) -> BTreeSet<SlibMemberId> {
    if dependencies.artifact_count() == 0 {
        return BTreeSet::new();
    }
    let mut available = BTreeMap::<(u64, Digest256), Vec<OdrBundle>>::new();
    for defined in dependencies.dependency_symbol_owners() {
        let dependency = dependencies
            .link(defined.producer())
            .expect("dependency symbol owners belong to the same artifact closure");
        let mut bundles = bundles(defined, dependency.definition_data().odr_members());
        for record in dependency.member_records() {
            if let Some(bundle) = bundles.remove(&record.id()) {
                let candidates = available
                    .entry((record.byte_length(), record.sha256()))
                    .or_default();
                if !candidates.contains(&bundle) {
                    candidates.push(bundle);
                }
            }
        }
    }
    let protected = exported_definitions(exports);
    let protected_members = link
        .materializations()
        .iter()
        .filter(|entry| {
            entry
                .scoop_lir_units()
                .is_some_and(|units| units.units().iter().any(|id| protected.contains(id)))
        })
        .map(|entry| entry.member())
        .collect::<BTreeSet<_>>();
    let bundles = bundles(link.defined_symbols(), odr);
    objects
        .iter()
        .filter_map(|object| {
            let record = object.record();
            if protected_members.contains(&record.id()) {
                return None;
            }
            let bundle = bundles.get(&record.id())?;
            let candidates = available.get(&(record.byte_length(), record.sha256()))?;
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

fn exported_definitions(
    exports: &LayoutAbiExportConstituentsV1,
) -> BTreeSet<ObjectDefinitionPlanId> {
    let mut definitions = BTreeSet::new();
    definitions.extend(
        exports
            .layouts()
            .records()
            .iter()
            .map(|record| record.identity().physical_definition().definition()),
    );
    definitions.extend(
        exports
            .descriptors()
            .records()
            .iter()
            .map(|record| record.physical_definition().definition()),
    );
    definitions.extend(
        exports
            .dispatch()
            .records()
            .iter()
            .map(|record| record.physical_definition().definition()),
    );
    definitions.extend(
        exports
            .callables()
            .records()
            .iter()
            .map(|record| record.physical_definition().definition()),
    );
    definitions.extend(
        exports
            .direct_callables()
            .exports()
            .iter()
            .map(|record| record.required_definition()),
    );
    for record in exports.shape_support().records() {
        let roles = record.roles();
        definitions.extend(
            [
                roles
                    .value_layout()
                    .available()
                    .map(|value| value.definition_plan()),
                roles
                    .ref_scan()
                    .available()
                    .map(|value| value.definition_plan()),
                roles
                    .type_descriptor()
                    .available()
                    .map(|value| value.definition_plan()),
                roles
                    .type_registration()
                    .available()
                    .map(|value| value.definition_plan()),
            ]
            .into_iter()
            .flatten(),
        );
        for shape in [
            roles.boxed_value(),
            roles.coroutine_step(),
            roles.coroutine_slot(),
        ]
        .into_iter()
        .filter_map(|role| role.available())
        {
            definitions.extend([
                shape.layout().definition_plan(),
                shape.scan().definition_plan(),
                shape.descriptor().definition_plan(),
                shape.registration().definition_plan(),
            ]);
        }
    }
    definitions
}
