use scoop_identity::ConeIdentity;
use scoop_wire::encode;

use super::*;

#[test]
fn six_empty_tables_have_the_fixed_manifest_projection() {
    let projection = CanonicalStrongRegistrationFingerprintSetV1 {
        producer: ConeIdentity::CORE,
        static_storages: Vec::new(),
        immortal_objects: Vec::new(),
        initialization_units: Vec::new(),
        type_registrations: Vec::new(),
        safepoints: Vec::new(),
        callables: Vec::new(),
    };

    assert_eq!(
        encode(&projection).unwrap(),
        vec![
            0xa6, 0x01, 0x80, 0x02, 0x80, 0x03, 0x80, 0x04, 0x80, 0x05, 0x80, 0x06, 0x80,
        ]
    );
}

#[test]
fn table_builder_sorts_and_rejects_duplicate_semantic_ids() {
    let first = entry(
        ConeIdentity::CORE,
        StrongRegistrationFingerprintV1::from_array([1; 32]),
    );
    let second = entry(
        ConeIdentity::SINGLE_FILE,
        StrongRegistrationFingerprintV1::from_array([2; 32]),
    );
    let sorted = canonicalize_table(
        StrongRegistrationFingerprintTableV1::Callable,
        vec![second, first],
    )
    .unwrap();
    assert_eq!(
        sorted
            .iter()
            .map(|entry| entry.semantic_id)
            .collect::<Vec<_>>(),
        vec![
            ConeIdentity::SINGLE_FILE.min(ConeIdentity::CORE),
            ConeIdentity::SINGLE_FILE.max(ConeIdentity::CORE)
        ]
    );

    assert_eq!(
        canonicalize_table(
            StrongRegistrationFingerprintTableV1::Callable,
            vec![first, first],
        ),
        Err(
            StrongRegistrationFingerprintProjectionError::DuplicateSemanticId(
                StrongRegistrationFingerprintTableV1::Callable
            )
        )
    );
}
