use super::*;
use crate::{ConeIdentity, Effect};
use scoop_wire::{encode, sha256};

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
fn generated_callable_hash_preserves_domain_framing() {
    let key = GeneratedCallableKey::Initialization {
        unit: PersistentInitializationUnitId(ConeIdentity::CORE.0),
        role: InitializationCallableRole::Initializer,
    };

    let domain = b"scoop-generated-callable-id-v1";
    let mut stream = (domain.len() as u64).to_le_bytes().to_vec();
    stream.extend(domain);
    stream.extend(encode(&key).unwrap());
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
fn generated_adapter_rejects_a_target_receiver() {
    let key = GeneratedCallableKey::DynamicFunctionAdapter {
        target: ExactCallableSignature::new(Effect::Ordinary, Some(exact()), vec![], exact()),
    };

    assert_eq!(
        PersistentGeneratedCallableId::from_key(&key),
        Err(GeneratedCallableIdentityError::TargetReceiverMustBeAbsent)
    );
}
