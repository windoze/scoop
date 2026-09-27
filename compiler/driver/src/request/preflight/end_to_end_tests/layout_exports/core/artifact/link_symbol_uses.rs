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
    path: &Path,
    core: &slib::AssembledCrossConeLayoutArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
) {
    let current = reader::open_link(artifact).identity();
    let mut dump = String::new();
    let mut cases = Vec::new();
    let mut runtime_dump = String::new();
    let mut runtime_cases = Vec::new();
    let mut coverage_dump = String::new();
    let mut code_dump = String::new();
    let complete = {
        let snapshots = [core.as_bytes().to_vec(), artifact.as_bytes().to_vec()];
        let mut sections = snapshots.iter().map(|bytes| {
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
                let (uses, categories) = partitions::check(proof);
                runtime_dump.push_str(&runtime::inspect(
                    if proof.provider() == current {
                        artifact
                    } else {
                        core
                    },
                    proof,
                ));
                coverage_dump.push_str(&coverage::inspect(proof));
                code_dump.push_str(&code::inspect(
                    if proof.provider() == current {
                        artifact
                    } else {
                        core
                    },
                    proof,
                ));
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
                partitions::dump(
                    &mut dump,
                    proof.provider() == current,
                    proof,
                    uses,
                    categories,
                );
            }
        })
        .unwrap();
    for (failure, mutation) in cases {
        rejection::mutation(core, artifact, profile, &mutation, failure);
        dump.push_str(&format!("reject {failure:?}\n"));
    }
    rejection::dependency_owner(core, artifact, profile);
    dump.push_str("reject DependencyDefinedMember\n");

    rejection::views(core, artifact, profile);
    runtime::check(path, core, artifact, profile, runtime_cases, runtime_dump);
    coverage::check(path, core, artifact, profile, coverage_dump);
    code::check(path, core, artifact, profile, code_dump);
    dump.push_str("reject CompileView\nreject MixedView\n");
    if std::env::var_os("SCOOP_UPDATE_LINK_SYMBOL_USES").is_some() {
        std::fs::write(path, &dump).unwrap();
    }
    assert_eq!(dump, std::fs::read_to_string(path).unwrap());
}
