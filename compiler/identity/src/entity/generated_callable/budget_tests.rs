use super::*;
use crate::{ConeIdentity, Effect};
use scoop_wire::{BudgetMeter, DecodeLimits, WirePath, encode, sha256};

fn exact() -> PersistentExactTypeId {
    PersistentExactTypeId(ConeIdentity::CORE.0)
}
fn adapter(parameters: usize) -> GeneratedCallableKey {
    GeneratedCallableKey::DynamicFunctionAdapter {
        target: ExactCallableSignature::new(
            Effect::Ordinary,
            None,
            vec![exact(); parameters],
            exact(),
        ),
    }
}

#[test]
fn initialization_hash_cost_includes_the_existing_domain_frame() {
    let key = GeneratedCallableKey::Initialization {
        unit: PersistentInitializationUnitId(ConeIdentity::CORE.0),
        role: InitializationCallableRole::Initializer,
    };
    let length = PersistentGeneratedCallableId::hash_stream_length(&key).unwrap();
    assert_eq!(length, 78);
    let domain = b"scoop-generated-callable-id-v1";
    let mut stream = (domain.len() as u64).to_le_bytes().to_vec();
    stream.extend(domain);
    stream.extend(encode(&key).unwrap());
    assert_eq!(stream.len() as u64, length);
    assert_eq!(
        PersistentGeneratedCallableId::from_key(&key)
            .unwrap()
            .to_string(),
        sha256(&stream).to_string()
    );
    assert_eq!(
        PersistentGeneratedCallableId::from_key(&adapter(0))
            .unwrap()
            .to_string(),
        "c57a69c85a9103908286eafb4e06ae42968689e0afab3ffeb0f49c138d9f7b69"
    );
}

#[test]
fn parameter_count_boundary_accounts_for_the_cbor_array_header() {
    let before = PersistentGeneratedCallableId::hash_stream_length(&adapter(23)).unwrap();
    let after = PersistentGeneratedCallableId::hash_stream_length(&adapter(24)).unwrap();
    assert_eq!(before, 86 + 23 * 34);
    assert_eq!(after - before, 35);
    for count in [0, 23, 24, 255, 256] {
        let key = adapter(count);
        assert_eq!(
            PersistentGeneratedCallableId::hash_stream_length(&key).unwrap(),
            38 + encode(&key).unwrap().len() as u64
        );
    }
}

#[test]
fn generated_hash_length_supports_an_inclusive_shared_sha_budget() {
    let length = PersistentGeneratedCallableId::hash_stream_length(&adapter(24)).unwrap();
    let mut measured = BudgetMeter::new(DecodeLimits::default());
    measured.charge_sha256(length, &WirePath::root()).unwrap();
    let required = measured.usage().validation_work_units;
    let mut exact = BudgetMeter::new(DecodeLimits {
        validation_work_units: required,
        ..DecodeLimits::default()
    });
    exact.charge_sha256(length, &WirePath::root()).unwrap();
    assert!(exact.charge_sha256(length, &WirePath::root()).is_err());
    let mut short = BudgetMeter::new(DecodeLimits {
        validation_work_units: required - 1,
        ..DecodeLimits::default()
    });
    assert!(short.charge_sha256(length, &WirePath::root()).is_err());
}

#[test]
fn hash_cost_rejects_the_same_invalid_target_receiver_as_identity_derivation() {
    let key = GeneratedCallableKey::DynamicFunctionAdapter {
        target: ExactCallableSignature::new(Effect::Ordinary, Some(exact()), vec![], exact()),
    };
    assert_eq!(
        PersistentGeneratedCallableId::hash_stream_length(&key),
        Err(GeneratedCallableIdentityError::TargetReceiverMustBeAbsent)
    );
    assert_eq!(
        PersistentGeneratedCallableId::from_key(&key),
        Err(GeneratedCallableIdentityError::TargetReceiverMustBeAbsent)
    );
}
