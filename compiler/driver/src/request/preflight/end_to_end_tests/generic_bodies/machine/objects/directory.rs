//! Exercise the shared manifest writer and reader with real generic objects.

use std::collections::BTreeSet;

use scoop_identity::OdrMemberId;
use scoop_slib::{
    CrossConeLayoutProductionManifestV1, DecodedCanonicalOdrMemberDirectoryV1,
    DecodedSingleConeProductionManifestV1, VerifiedCodeFingerprintV2,
};
use scoop_wire::{decode_canonical, encode};

pub(super) fn check(code: &VerifiedCodeFingerprintV2, expected: &BTreeSet<OdrMemberId>) {
    let projection = code.production().projection();
    let directory = projection.odr_members();
    let actual = directory
        .groups()
        .iter()
        .flat_map(|group| group.members())
        .map(|entry| entry.member())
        .collect::<BTreeSet<_>>();
    assert_eq!(&actual, expected);
    assert!(!actual.is_empty());
    let bytes = encode(directory).unwrap();
    decode_canonical::<DecodedCanonicalOdrMemberDirectoryV1>(&bytes)
        .unwrap()
        .validate_against(directory)
        .unwrap();
    let projection_bytes = encode(projection).unwrap();
    assert_eq!(projection_bytes[0], 0xa7);
    let mut suffix = vec![11];
    suffix.extend_from_slice(&bytes);
    suffix.push(12);
    suffix.extend_from_slice(&encode(&projection.optimization()).unwrap());
    assert!(projection_bytes.ends_with(&suffix));

    let manifest = CrossConeLayoutProductionManifestV1::from_verified_code(code.clone());
    let bytes = encode(&manifest).unwrap();
    assert_eq!(bytes[0], 0xa2);
    let decoded = decode_canonical::<DecodedSingleConeProductionManifestV1>(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
}
