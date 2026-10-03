//! Code replay checks both fingerprint copies and every remaining manifest field.

use super::super::link_archive;
use super::*;

mod mutation;
mod rejection;

#[derive(Clone, Copy, Debug)]
enum Case {
    Distribution,
    OutputBranch,
    ManifestCode,
    OuterCode,
    MatchingWrongCode,
    NativeContractExtra,
    NativeContractMissing,
    NativeContractDuplicate,
    NativeContractOrder,
    NativeContractFingerprint,
    NativeLibraryExtra,
}

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

pub(super) fn check(
    core: &slib::AssembledCrossConeLayoutArtifactV1,
    artifact: &slib::AssembledCrossConeLayoutArtifactV1,
    profile: &lir::CBridgeToolchainProfileV1,
) {
    let core_manifest = encode(reader::open_link(core).production_manifest_wire()).unwrap();
    let contracts = wire::field_range(&core_manifest, 8);
    let seed = wire::array_parts(&core_manifest[contracts])[0].to_vec();
    for dependency in [false, true] {
        let source = if dependency { core } else { artifact };
        let manifest = encode(reader::open_link(source).production_manifest_wire()).unwrap();
        let contracts = wire::field_range(&manifest, 8);
        let count = wire::array_parts(&manifest[contracts]).len();
        let mut cases = vec![
            Case::Distribution,
            Case::OutputBranch,
            Case::ManifestCode,
            Case::OuterCode,
            Case::MatchingWrongCode,
            Case::NativeContractExtra,
            Case::NativeLibraryExtra,
        ];
        if count > 0 {
            cases.extend([
                Case::NativeContractMissing,
                Case::NativeContractDuplicate,
                Case::NativeContractFingerprint,
            ]);
        }
        if count > 1 {
            cases.push(Case::NativeContractOrder);
        }
        for case in cases {
            rejection::check(core, artifact, profile, dependency, case, &seed);
        }
    }
}
