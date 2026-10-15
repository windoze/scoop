//! A physical reference cannot replace the logical Any identity in a call.

use super::*;

pub(in super::super) fn check_link(
    provider: &slib::AssembledCrossConeLayoutArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutArtifactV1,
) {
    use super::super::lir_dependencies::reader as artifact_reader;
    let source = artifact_reader::open_link(artifact)
        .into_shared_sections()
        .unwrap();
    let current = source.identity();
    let expected = source.semantic_fingerprints().code();
    artifact_reader::read_link(provider, artifact)
        .read_definition_data()
        .map(|closure| {
            assert_eq!(closure.dependency_first().len(), 2);
            let (physical, proof) = closure.artifact(current).unwrap();
            assert_eq!(physical.identity(), proof.provider());
            assert_eq!(
                expected,
                proof
                    .summary()
                    .manifest_summary()
                    .semantic_fingerprints()
                    .code()
            );
        })
        .unwrap()
}
