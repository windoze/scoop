use scoop_identity::{CborIdentityRecord, CoreBuiltinNominal, ExactTypeKey};
use scoop_wire::{DecodeLimits, WireDecode, decode_canonical, encode};

use super::*;

fn exact() -> PersistentExactTypeId {
    CborIdentityRecord::from_key(ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap()
    .id()
}

fn reference_bytes(tag: u8, provider: Option<[u8; 32]>, target: [u8; 32]) -> Vec<u8> {
    let mut bytes = vec![
        if provider.is_some() { 0xa3 } else { 0xa2 },
        0,
        tag,
        1,
        0x58,
        0x20,
    ];
    if let Some(provider) = provider {
        bytes.extend_from_slice(&provider);
        bytes.extend_from_slice(&[2, 0x58, 0x20]);
    }
    bytes.extend_from_slice(&target);
    bytes
}

fn round_trip<T: WireDecode + WireEncode>(bytes: &[u8]) {
    let decoded: T = decode_canonical(bytes, DecodeLimits::default()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
}

#[test]
fn descriptor_references_preserve_legacy_payloads_and_add_provider_bytes() {
    let exact = exact();
    for (reference, tag, provider) in [
        (StrongTypeDescriptorRefV2::Local(exact), 1, None),
        (StrongTypeDescriptorRefV2::CoreExternal(exact), 2, None),
        (
            StrongTypeDescriptorRefV2::DependencyExternal {
                provider: ConeIdentity::CORE,
                exact,
            },
            3,
            Some(*ConeIdentity::CORE.as_array()),
        ),
    ] {
        let expected = reference_bytes(tag, provider, *exact.as_array());
        assert_eq!(encode(&reference).unwrap(), expected);
        assert_eq!(reference.exact_type(), exact);
        round_trip::<DecodedStrongTypeDescriptorRefV2>(&expected);
    }
}

#[test]
fn optional_absent_keeps_its_explicit_zero_marker() {
    let exact = exact();
    assert_eq!(
        encode(&OptionalStrongTypeDescriptorRefV2::Absent).unwrap(),
        [0xa2, 0, 1, 1, 0]
    );
    round_trip::<DecodedOptionalStrongTypeDescriptorRefV2>(&[0xa2, 0, 1, 1, 0]);
    for (reference, tag, provider) in [
        (OptionalStrongTypeDescriptorRefV2::Local(exact), 2, None),
        (
            OptionalStrongTypeDescriptorRefV2::CoreExternal(exact),
            3,
            None,
        ),
        (
            OptionalStrongTypeDescriptorRefV2::DependencyExternal {
                provider: ConeIdentity::CORE,
                exact,
            },
            4,
            Some(*ConeIdentity::CORE.as_array()),
        ),
    ] {
        let expected = reference_bytes(tag, provider, *exact.as_array());
        assert_eq!(encode(&reference).unwrap(), expected);
        round_trip::<DecodedOptionalStrongTypeDescriptorRefV2>(&expected);
    }
}

#[test]
fn dispatch_retains_body_kind_and_runtime_encoding() {
    for (tag, provider) in [(1, None), (2, None), (4, Some([0x11; 32]))] {
        round_trip::<DecodedStrongTypeDispatchCallableRefV2>(&reference_bytes(
            tag, provider, [0x22; 32],
        ));
    }
    let runtime = StrongTypeDispatchCallableRefV2::Runtime(RuntimeFunction::NoGc(
        crate::NoGcRuntimeFunction::Trap,
    ));
    let bytes = [0xa2, 0, 3, 1, 0xa2, 1, 2, 2, 9];
    assert_eq!(encode(&runtime).unwrap(), bytes);
    round_trip::<DecodedStrongTypeDispatchCallableRefV2>(&bytes);
}

#[test]
fn each_new_dependency_branch_is_a_closed_product() {
    let mut cases = vec![vec![0xa1, 0, 3], reference_bytes(3, None, [0x22; 32])];
    let mut extra = reference_bytes(3, Some([0x11; 32]), [0x22; 32]);
    extra[0] = 0xa4;
    extra.extend_from_slice(&[3, 0]);
    cases.push(extra);
    cases.push(reference_bytes(4, Some([0x11; 32]), [0x22; 32]));
    for bytes in cases {
        assert!(
            decode_canonical::<DecodedStrongTypeDescriptorRefV2>(&bytes, DecodeLimits::default())
                .is_err()
        );
    }
    for bytes in [&[0xa1, 0, 1][..], &[0xa2, 0, 1, 1, 1][..]] {
        assert!(
            decode_canonical::<DecodedOptionalStrongTypeDescriptorRefV2>(
                bytes,
                DecodeLimits::default()
            )
            .is_err()
        );
    }
    assert!(
        decode_canonical::<DecodedStrongTypeDispatchCallableRefV2>(
            &[0xa2, 0, 3, 1, 0xa2, 1, 2, 2, 0x18, 0xff],
            DecodeLimits::default()
        )
        .is_err()
    );
}

#[test]
fn unknown_provider_is_not_promoted_by_identity_resolution() {
    struct Reject;
    impl scoop_identity::PersistentIdResolver<ConeIdentity> for Reject {
        type Error = &'static str;
        fn resolve(
            &mut self,
            _: scoop_identity::DecodedPersistentId<ConeIdentity>,
        ) -> Result<ConeIdentity, Self::Error> {
            Err("provider absent")
        }
    }
    impl scoop_identity::PersistentIdResolver<PersistentExactTypeId> for Reject {
        type Error = &'static str;
        fn resolve(
            &mut self,
            _: scoop_identity::DecodedPersistentId<PersistentExactTypeId>,
        ) -> Result<PersistentExactTypeId, Self::Error> {
            Ok(exact())
        }
    }
    let bytes = reference_bytes(3, Some([0x11; 32]), *exact().as_array());
    let decoded: DecodedStrongTypeDescriptorRefV2 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(decoded.resolve(&mut Reject), Err("provider absent"));
}
