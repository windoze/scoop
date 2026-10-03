//! Replay real Link bytes and reject hash-valid physical corruptions.

use super::lir_dependencies::reader;
use super::*;
use scoop_slib as slib;
use std::collections::BTreeMap;

mod mutations;
mod projection;
mod rejection;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Failure {
    Envelope,
    CBridgeEnvelope,
    Symbols,
    Relocations,
    Stackmaps,
    Safepoints,
    Callables,
    Types,
    Immortals,
    Storages,
    Initializations,
    AtomRange,
    DigestIntent,
}

enum Mutation {
    Object {
        member: slib::SlibMemberId,
        offset: usize,
        value: u8,
    },
    Projection(Failure),
}

pub(super) fn check(
    core: &slib::AssembledCrossConeLayoutArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
) {
    let mut cases = BTreeMap::new();
    let mut bridge_providers = Vec::new();
    reader::read_link(core, artifact)
        .replay_link_object_contents(profile)
        .map(|closure| {
            assert_eq!(closure.dependency_first().len(), 2);
            for (physical, proof) in closure.dependency_first() {
                assert_eq!(proof.provider(), physical.identity());
                assert!(physical.link_sections().is_some());
                let source = if proof.provider() == closure.physical_imports().current() {
                    artifact
                } else {
                    core
                };
                assert_eq!(
                    closure.artifact(proof.provider()).unwrap().0.identity(),
                    proof.provider()
                );
                check_objects(source, proof);
                if proof.generated_objects().len() != 0 {
                    bridge_providers.push(proof.provider());
                }
                let strong = physical.lir_strong_production();
                assert_eq!(proof.callables().plan(), strong.callable_registrations());
                assert_eq!(proof.types().plan(), strong.type_registrations());
                assert_eq!(proof.immortals().plan(), strong.immortal_registrations());
                assert_eq!(
                    proof.storages().plan(),
                    strong.static_storage_registrations()
                );
                assert_eq!(
                    proof.initializations().plan(),
                    strong.initialization_registrations()
                );
                assert_eq!(proof.safepoints().plan(), strong.safepoint_registrations());
                for (failure, mutation) in mutations::cases(proof) {
                    cases.entry(failure).or_insert((proof.provider(), mutation));
                }
            }
        })
        .unwrap();
    for failure in [
        Failure::Envelope,
        Failure::CBridgeEnvelope,
        Failure::Symbols,
        Failure::Relocations,
        Failure::Stackmaps,
        Failure::Safepoints,
        Failure::Callables,
        Failure::Types,
        Failure::Immortals,
        Failure::Storages,
        Failure::Initializations,
    ] {
        assert!(
            cases.contains_key(&failure),
            "fixture must exercise {failure:?}"
        );
    }
    let current = reader::open_link(artifact).identity();
    for failure in [Failure::AtomRange, Failure::DigestIntent] {
        cases.insert(failure, (current, Mutation::Projection(failure)));
    }
    for (failure, (provider, mutation)) in cases {
        rejection::mutation(core, artifact, profile, provider, &mutation, failure);
    }
    rejection::views_and_profile(core, artifact, profile, bridge_providers[0]);
}

fn check_objects(
    source: &slib::AssembledCrossConeLayoutArtifactV1,
    proof: &slib::ReplayedLayoutLinkObjectContentsV1,
) {
    let payloads = super::link_archive::payloads(source);
    for object in proof.objects().objects() {
        assert_eq!(object.final_bytes(), payloads[&object.member()]);
        let mut expected = object.final_bytes().to_vec();
        for patch in proof
            .patch_sites()
            .sites()
            .iter()
            .filter(|patch| patch.member() == object.member())
        {
            let start = patch.checked_offset() as usize;
            expected[start..start + 32].fill(0);
        }
        assert_eq!(expected, object.provisional_bytes());
    }
    for object in proof.generated_objects() {
        assert_eq!(object.bytes(), payloads[&object.member()]);
    }
}
