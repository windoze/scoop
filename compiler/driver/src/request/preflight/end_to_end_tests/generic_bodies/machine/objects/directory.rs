//! Exercise the shared manifest writer and reader with real generic objects.

use std::collections::BTreeSet;

use scoop_identity::OdrMemberId;
use scoop_slib::{
    CrossConeLayoutProductionManifestV1, DecodedCanonicalOdrMemberDirectoryV1,
    DecodedSingleConeProductionManifestV1, RegistrationFingerprintV1,
    SingleConeProductionManifestValidationError, VerifiedCodeFingerprintV2,
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
    let registrations = code
        .production()
        .link_objects()
        .final_objects()
        .runtime_images()
        .fingerprint()
        .registrations();
    let strong = projection.strong_registration_set();
    assert_eq!(
        strong
            .callables()
            .iter()
            .map(|entry| (entry.semantic_id(), entry.fingerprint()))
            .collect::<Vec<_>>(),
        registrations
            .callables()
            .fingerprints()
            .iter()
            .filter_map(|entry| match entry.registration() {
                RegistrationFingerprintV1::Strong(value) => Some((entry.body(), value)),
                RegistrationFingerprintV1::Odr(_) => None,
            })
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        strong
            .safepoints()
            .iter()
            .map(|entry| (entry.semantic_id(), entry.fingerprint()))
            .collect::<Vec<_>>(),
        registrations
            .safepoints()
            .fingerprints()
            .iter()
            .filter_map(|entry| match entry.registration() {
                RegistrationFingerprintV1::Strong(value) => Some((entry.site(), value)),
                RegistrationFingerprintV1::Odr(_) => None,
            })
            .collect::<Vec<_>>(),
    );
    let bytes = encode(directory).unwrap();
    decode_canonical::<DecodedCanonicalOdrMemberDirectoryV1>(&bytes)
        .unwrap()
        .validate_against(directory)
        .unwrap();
    let projection_bytes = encode(projection).unwrap();
    assert_eq!(projection_bytes[0], 0xa7);
    assert_eq!(
        projection_bytes[projection_bytes.len() - bytes.len() - 1],
        11
    );
    assert!(projection_bytes.ends_with(&bytes));

    let manifest = CrossConeLayoutProductionManifestV1::from_verified_code(code.clone());
    let bytes = encode(&manifest).unwrap();
    assert_eq!(bytes[0], 0xab);
    let decoded = decode_canonical::<DecodedSingleConeProductionManifestV1>(&bytes).unwrap();
    decoded
        .replay_runtime_projection(
            projection.runtime_registration_projection(),
            code.production()
                .link_objects()
                .final_objects()
                .runtime_images()
                .fingerprint(),
        )
        .unwrap();
    assert_eq!(decoded.validate_layout(code).unwrap(), manifest);

    let entry = directory.groups()[0].members()[0];
    let entry_bytes = encode(&entry).unwrap();
    let positions = bytes
        .windows(entry_bytes.len())
        .enumerate()
        .filter_map(|(i, value)| (value == entry_bytes).then_some(i))
        .collect::<Vec<_>>();
    let [start] = positions.as_slice() else {
        panic!("a member appears exactly once in the manifest");
    };
    for digest in [entry.abi().as_array(), entry.definition().as_array()] {
        let offset = entry_bytes
            .windows(32)
            .position(|value| value == digest)
            .unwrap();
        let mut changed = bytes.clone();
        changed[start + offset] ^= 1;
        assert!(matches!(
            decode_canonical::<DecodedSingleConeProductionManifestV1>(&changed)
                .unwrap()
                .validate_layout(code),
            Err(SingleConeProductionManifestValidationError::ProjectionMismatch)
        ));
    }
}
