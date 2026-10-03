//! Finalized runtime bytes and metadata are independently rebuilt on read.

use super::super::link_archive::{self, Rewrite};
use super::*;
use object::{Object as _, ObjectSection as _};
use std::collections::BTreeMap;

mod rejection;

#[derive(Clone, Copy, Debug)]
pub(super) enum RuntimeMutation {
    Field(u32),
    Body(slib::SlibMemberId, u64),
    Patch(slib::SlibMemberId, u64),
}

pub(super) fn inspect(
    artifact: &slib::AssembledCrossConeLayoutArtifactV1,
    proof: &slib::ReplayedLayoutLinkSymbolUsesV1,
) {
    let payloads = link_archive::payloads(artifact);
    let finalized = proof.final_objects();
    for object in finalized.objects() {
        assert_eq!(object.bytes(), payloads[&object.member()]);
    }
    let registrations = finalized.runtime_images().fingerprint().registrations();
    assert_eq!(registrations.producer(), proof.provider());
}

pub(super) fn cases(proof: &slib::ReplayedLayoutLinkSymbolUsesV1) -> Vec<RuntimeMutation> {
    let mut by_role = BTreeMap::new();
    for patch in proof.object_contents().patch_sites().sites() {
        by_role.entry(patch.semantic_field_role()).or_insert(*patch);
    }
    let mut cases = (3..=6)
        .map(RuntimeMutation::Field)
        .chain(
            by_role
                .into_values()
                .map(|patch| RuntimeMutation::Patch(patch.member(), patch.checked_offset())),
        )
        .collect::<Vec<_>>();
    cases.push(body_mutation(proof));
    cases
}

fn body_mutation(proof: &slib::ReplayedLayoutLinkSymbolUsesV1) -> RuntimeMutation {
    for candidate in proof.object_contents().objects().objects() {
        let file = object::File::parse(candidate.final_bytes()).unwrap();
        for section in file
            .sections()
            .filter(|section| section.kind() == object::SectionKind::Text)
        {
            // A RET has no relocation bits. Changing the instruction must change
            // the actual callable-body and runtime-image fingerprints.
            if let Some(index) = section
                .data()
                .unwrap()
                .chunks_exact(4)
                .position(|instruction| instruction == [0xc0, 0x03, 0x5f, 0xd6])
            {
                return RuntimeMutation::Body(
                    candidate.member(),
                    section.file_range().unwrap().0 + (index as u64) * 4,
                );
            }
        }
    }
    panic!("native callable fixture contains an AArch64 return");
}

pub(super) fn check(
    core: &slib::AssembledCrossConeLayoutArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
    cases: Vec<RuntimeMutation>,
) {
    for mutation in cases {
        rejection::check(core, artifact, profile, mutation);
    }
}
