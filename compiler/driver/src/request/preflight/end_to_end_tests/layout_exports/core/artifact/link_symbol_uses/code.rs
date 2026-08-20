use super::*;

pub(super) fn inspect(
    artifact: &slib::AssembledCrossConeLayoutArtifactV1,
    proof: &slib::ReplayedLayoutLinkSymbolUsesV1,
) {
    let expected = reader::open_link(artifact)
        .into_shared_sections()
        .unwrap()
        .semantic_fingerprints()
        .code();
    assert_eq!(
        expected,
        slib::FingerprintAvailability::Available(proof.code_fingerprint())
    );
}
