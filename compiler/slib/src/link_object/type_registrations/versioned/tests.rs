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
fn old_descriptor_references_and_relocations_keep_their_bytes() {
    let exact = exact();
    for (old, new, tag) in [
        (
            StrongTypeDescriptorRefV1::Local(exact),
            StrongTypeDescriptorRefV2::Local(exact),
            1u32,
        ),
        (
            StrongTypeDescriptorRefV1::CoreExternal(exact),
            StrongTypeDescriptorRefV2::CoreExternal(exact),
            2,
        ),
    ] {
        let mut expected = tag.to_le_bytes().to_vec();
        expected.extend_from_slice(exact.as_array());
        assert_eq!(bytes(old), expected);
        assert_eq!(bytes(new), expected);
        assert_eq!(
            encode_runtime(&old.canonical_relocation(80)).unwrap(),
            encode_runtime(&new.canonical_relocation(80)).unwrap(),
        );
    }
}

#[test]
fn dependency_descriptor_semantics_and_relocations_retain_provider_and_exact_type() {
    let exact = exact();
    let mut encoded = Vec::new();
    for provider in [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE] {
        let reference = StrongTypeDescriptorRefV2::DependencyExternal { provider, exact };
        let mut expected = 3u32.to_le_bytes().to_vec();
        expected.extend_from_slice(provider.as_array());
        expected.extend_from_slice(exact.as_array());
        assert_eq!(bytes(reference), expected);

        let actual = encode_runtime(&reference.canonical_relocation(80)).unwrap();
        let mut target = 12u32.to_le_bytes().to_vec();
        target.extend_from_slice(provider.as_array());
        target.extend_from_slice(&4u32.to_le_bytes()); // TypeDescriptor shape subject.
        target.extend_from_slice(exact.as_array());
        assert_eq!(&actual[actual.len() - target.len()..], target);
        assert_ne!(
            actual,
            encode_runtime(
                &StrongTypeDescriptorRefV2::CoreExternal(exact).canonical_relocation(80)
            )
            .unwrap(),
        );
        encoded.push(actual);
    }
    assert_ne!(encoded[0], encoded[1]);
}

#[test]
fn dispatch_fingerprint_encoding_preserves_old_tags_and_binds_new_provider() {
    use crate::link_object::stackmap_normalization::verification::tests::support::{
        Corruption, semantic,
    };
    let body = semantic::inputs(Corruption::None).module.functions[0]
        .callable_body
        .id();
    for (old, current, tag) in [
        (
            StrongTypeDispatchCallableRefV1::Local(body),
            StrongTypeDispatchCallableRefV2::Local(body),
            1u32,
        ),
        (
            StrongTypeDispatchCallableRefV1::CoreExternal(body),
            StrongTypeDispatchCallableRefV2::CoreExternal(body),
            2,
        ),
    ] {
        let mut expected = tag.to_le_bytes().to_vec();
        expected.extend_from_slice(body.as_array());
        let mut old_bytes = RuntimeEncoder::new();
        let mut current_bytes = RuntimeEncoder::new();
        old.runtime_encode(&mut old_bytes).unwrap();
        current.runtime_encode(&mut current_bytes).unwrap();
        assert_eq!(old_bytes.into_bytes(), expected);
        assert_eq!(current_bytes.into_bytes(), expected);
    }
    for provider in [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE] {
        let mut encoder = RuntimeEncoder::new();
        StrongTypeDispatchCallableRefV2::DependencyExternal { provider, body }
            .runtime_encode(&mut encoder)
            .unwrap();
        let mut expected = 4u32.to_le_bytes().to_vec();
        expected.extend_from_slice(provider.as_array());
        expected.extend_from_slice(body.as_array());
        assert_eq!(encoder.into_bytes(), expected);
    }
}
