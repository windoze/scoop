use super::*;
use scoop_wire::{decode_canonical, encode};

#[test]
fn alias_section_has_a_fixed_empty_wire_and_rejects_other_shapes() {
    let bytes = [0xa1, 1, 0x80];
    assert_eq!(encode(&LirLinkSupportSectionV1::default()).unwrap(), bytes);
    assert_eq!(
        encode(&decode_canonical::<DecodedLirLinkSupportSectionV1>(&bytes).unwrap()).unwrap(),
        bytes
    );
    for invalid in [
        &[0xa0][..],
        &[0xa1, 2, 0x80],
        &[0xa2, 1, 0x80, 2, 0x80],
        &[0xa1, 1, 0xa0],
    ] {
        assert!(decode_canonical::<DecodedLirLinkSupportSectionV1>(invalid).is_err());
    }
}

#[test]
fn link_support_is_required_for_both_consumers_and_only_contributes_to_code() {
    let capability = crate::lir_link_support_capability();
    let contract = crate::CapabilityContractRegistry::contract(&capability).unwrap();
    assert_eq!(
        contract.required_for(),
        crate::MemberPurposeSet::COMPILE_AND_LINK
    );
    assert_eq!(
        contract.sinks(),
        crate::FingerprintSinkSet::CODE.union(crate::FingerprintSinkSet::LINK_VALIDATION_ONLY)
    );
    assert!(
        crate::ArtifactCapabilityProfile::CROSS_CONE_GENERIC
            .descriptor()
            .required_lir()
            .contains(&capability)
    );
}
