use super::*;

#[test]
fn native_target_contract_can_reference_a_dependency_source_contract() {
    let (decoded, provider, bytes) = native_contract_fixture(None);
    let mut identities = dependency_graph(&decoded, &provider);
    let validated = decoded
        .validate(ConeIdentity::SINGLE_FILE, &mut identities)
        .unwrap();

    assert_eq!(validated.counts().native_contracts, 1);
    assert_eq!(encode(&validated).unwrap(), bytes);
}

#[test]
fn unused_dependency_source_contract_does_not_require_a_target_contract() {
    let (_, provider, _) = native_contract_fixture(None);
    let bytes = encode(&CanonicalLirFoundation::empty()).unwrap();
    let decoded = decode_canonical::<DecodedLirFoundation>(&bytes).unwrap();
    let mut identities = dependency_graph(&decoded, &provider);
    let validated = decoded
        .validate(ConeIdentity::SINGLE_FILE, &mut identities)
        .unwrap();

    assert_eq!(validated.counts().native_contracts, 0);
    assert_eq!(encode(&validated).unwrap(), bytes);
}

fn dependency_graph(
    decoded: &DecodedLirFoundation,
    provider: &ValidatedIdentityGraph,
) -> ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_authority(ConeIdentity::SINGLE_FILE)
        .unwrap();
    decoded.register_identities(&mut pending).unwrap();
    pending
        .register_external_graph_authorities(provider)
        .unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    pending.finish().unwrap()
}
