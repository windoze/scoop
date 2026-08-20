use super::*;
use scoop_identity::{CoreBuiltinNominal, ExactTypeKey};
use scoop_wire::encode_runtime;

fn exact() -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap()
}

fn bytes(reference: impl LinkDescriptorReference) -> Vec<u8> {
    let mut encoder = RuntimeEncoder::new();
    reference.runtime_encode(&mut encoder).unwrap();
    encoder.into_bytes()
}

#[test]
fn local_descriptor_references_and_relocations_keep_their_bytes() {
    let exact = exact();
    let (old, new, tag) = (
        StrongTypeDescriptorRefV1::Local(exact),
        StrongTypeDescriptorRefV2::Local(exact),
        1u32,
    );
    let mut expected = tag.to_le_bytes().to_vec();
    expected.extend_from_slice(exact.as_array());
    assert_eq!(bytes(old), expected);
    assert_eq!(bytes(new), expected);
    assert_eq!(
        encode_runtime(&old.canonical_relocation(80)).unwrap(),
        encode_runtime(&new.canonical_relocation(80)).unwrap(),
    );
}

#[test]
fn dependency_descriptor_keeps_provider_in_semantics_and_exact_identity_in_relocations() {
    let exact = exact();
    let local =
        encode_runtime(&StrongTypeDescriptorRefV2::Local(exact).canonical_relocation(80)).unwrap();
    for provider in [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE] {
        let reference = StrongTypeDescriptorRefV2::DependencyExternal { provider, exact };
        let mut expected = 3u32.to_le_bytes().to_vec();
        expected.extend_from_slice(provider.as_array());
        expected.extend_from_slice(exact.as_array());
        assert_eq!(bytes(reference), expected);

        let actual = encode_runtime(&reference.canonical_relocation(80)).unwrap();
        assert_eq!(actual, local);
        let legacy = StrongTypeDescriptorRefV1::DependencyExternal { provider, exact };
        assert_eq!(bytes(legacy), expected);
        assert_eq!(
            actual,
            encode_runtime(&legacy.canonical_relocation(80)).unwrap()
        );
    }
}

#[test]
fn dispatch_fingerprint_encoding_preserves_local_tags_and_binds_external_provider() {
    use crate::link_object::stackmap_normalization::verification::tests::support::{
        Corruption, semantic,
    };
    let body = semantic::inputs(Corruption::None).module.functions[0]
        .callable_body
        .id();
    let (old, current, tag) = (
        StrongTypeDispatchCallableRefV1::Local(body),
        StrongTypeDispatchCallableRefV2::Local(body),
        1u32,
    );
    let mut expected = tag.to_le_bytes().to_vec();
    expected.extend_from_slice(body.as_array());
    let mut old_bytes = RuntimeEncoder::new();
    let mut current_bytes = RuntimeEncoder::new();
    old.runtime_encode(&mut old_bytes).unwrap();
    current.runtime_encode(&mut current_bytes).unwrap();
    assert_eq!(old_bytes.into_bytes(), expected);
    assert_eq!(current_bytes.into_bytes(), expected);
    for provider in [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE] {
        let mut encoder = RuntimeEncoder::new();
        StrongTypeDispatchCallableRefV2::DependencyExternal { provider, body }
            .runtime_encode(&mut encoder)
            .unwrap();
        let mut expected = 4u32.to_le_bytes().to_vec();
        expected.extend_from_slice(provider.as_array());
        expected.extend_from_slice(body.as_array());
        assert_eq!(encoder.into_bytes(), expected);
        let mut legacy_encoder = RuntimeEncoder::new();
        StrongTypeDispatchCallableRefV1::DependencyExternal { provider, body }
            .runtime_encode(&mut legacy_encoder)
            .unwrap();
        assert_eq!(legacy_encoder.into_bytes(), expected);
    }
}
