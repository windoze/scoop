//! Actual defined owners and disjoint undefined uses survive independent reads.

use super::lir_dependencies::reader;
use super::*;
use scoop_slib as slib;

mod code;
mod coverage;
mod mutations;
mod partitions;
mod rejection;
mod runtime;
mod wire;

#[derive(Clone, Copy, Debug)]
enum Failure {
    DefinedMissing,
    DefinedDuplicate,
    DefinedMember,
    LegacyMissing,
    LegacyWidth,
    OrdinaryMissing,
    OrdinaryDuplicate,
    OrdinaryIndex,
    ShapeMissing,
    ShapeDuplicate,
    ShapeIndex,
    UnknownRuntime,
    WrongNativeSymbol,
}

enum Mutation {
    Projection(Failure),
    Object {
        member: slib::SlibMemberId,
        offset: usize,
    },
}

pub(super) fn check(
    core: &slib::AssembledCrossConeLayoutArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
) {
    let current = reader::open_link(artifact).identity();
    let mut cases = Vec::new();
    let mut runtime_cases = Vec::new();
    let complete = {
        let archives = [core.as_bytes().to_vec(), artifact.as_bytes().to_vec()];
        let mut sections = archives.iter().map(|bytes| {
            slib::DecodedSlibEnvelope::open(bytes, artifact.target_selection())
                .unwrap()
                .validate_graph()
                .unwrap()
                .decode_cross_cone_layout_link_sections()
                .unwrap()
                .into_shared_sections()
                .unwrap()
        });
        let core = sections.next().unwrap();
        let current = sections.next().unwrap();
        reader::read_sections(core, current).replay_link_symbol_uses(profile)
    };
    // The complete result remains usable after both raw archive buffers are dropped.
    complete
        .map(|closure| {
            assert_eq!(closure.dependency_first().len(), 2);
            for (physical, proof) in closure.dependency_first() {
                assert_eq!(physical.identity(), proof.provider());
                assert_eq!(physical.manifest().cone().identity(), physical.identity());
                assert_eq!(physical.lir_foundation().producer(), physical.identity());
                assert_eq!(
                    closure.artifact(proof.provider()).unwrap().1.provider(),
                    proof.provider()
                );
                let partitions = proof.undefined_partitions();
                partitions::check(proof);
                runtime::inspect(
                    if proof.provider() == current {
                        artifact
                    } else {
                        core
                    },
                    proof,
                );
                coverage::inspect(proof);
                code::inspect(
                    if proof.provider() == current {
                        artifact
                    } else {
                        core
                    },
                    proof,
                );
                let owners = partitions.cross_cone().dependency_owners();
                if proof.provider() == current {
                    assert_eq!(owners.len(), 1);
                    assert_eq!(
                        owners[0],
                        *closure
                            .artifact(owners[0].producer())
                            .unwrap()
                            .1
                            .defined_symbols()
                    );
                    cases = mutations::cases(proof);
                    runtime_cases = runtime::cases(proof);
                } else {
                    assert!(owners.is_empty());
                }
            }
        })
        .unwrap();
    for (failure, mutation) in cases {
        rejection::mutation(core, artifact, profile, &mutation, failure);
    }
    rejection::dependency_owner(core, artifact, profile);

    rejection::views(core, artifact, profile);
    runtime::check(core, artifact, profile, runtime_cases);
    coverage::check(core, artifact, profile);
    code::check(core, artifact, profile);
}
