use super::super::link_archive;
use super::*;

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
