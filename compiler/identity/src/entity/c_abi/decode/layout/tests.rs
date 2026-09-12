use std::num::NonZeroU64;

use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::{
    DecodedCLayoutOverride, DecodedCanonicalCAbiLayout, DecodedCanonicalCAbiLayoutFingerprintRecord,
};
use crate::{
    CLayoutByteAlignment, CLayoutOverride, CanonicalCAbiLayout, CanonicalCAbiLayoutField,
    CanonicalCAbiLayoutFingerprint, CanonicalCAbiLayoutFingerprintRecord,
    CanonicalCAbiSignatureFingerprint, CanonicalCStorageType, PersistentExactTypeId,
    PersistentFieldId, PersistentIdMismatch, PersistentIdResolver,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

struct Resolver;

macro_rules! id_resolver {
    ($id:ty, $expected:expr) => {
        impl PersistentIdResolver<$id> for Resolver {
            type Error = ResolutionError;

            fn resolve(&mut self, id: crate::DecodedPersistentId<$id>) -> Result<$id, Self::Error> {
                id.verify($expected)
                    .map_err(|_: PersistentIdMismatch<$id>| ResolutionError)
            }
        }
    };
}

id_resolver!(PersistentExactTypeId, exact_type());
id_resolver!(PersistentFieldId, field());
id_resolver!(CanonicalCAbiSignatureFingerprint, signature());
id_resolver!(CanonicalCAbiLayoutFingerprint, referenced_layout());

#[test]
fn c_abi_layout_round_trips_and_resolves_fields() {
    let layout = layout();
    let decoded = decode_canonical::<DecodedCanonicalCAbiLayout>(
        &encode(&layout).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();

    assert_eq!(decoded.resolve(&mut Resolver).unwrap(), layout);
}

#[test]
fn c_abi_layout_fingerprint_record_round_trips_and_verifies_hash() {
    let record = CanonicalCAbiLayoutFingerprintRecord::new(layout()).unwrap();
    let decoded = decode_canonical::<DecodedCanonicalCAbiLayoutFingerprintRecord>(
        &encode(&record).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(
        decoded.candidate_fingerprint().unwrap(),
        record.fingerprint()
    );
    assert_eq!(decoded.resolve(&mut Resolver).unwrap(), record);

    let mut bytes = encode(&record).unwrap();
    assert_eq!(&bytes[..4], &[0xa2, 0x01, 0x58, 0x20]);
    bytes[4] ^= 1;
    let decoded = decode_canonical::<DecodedCanonicalCAbiLayoutFingerprintRecord>(
        &bytes,
        DecodeLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        decoded.resolve(&mut Resolver),
        Err(super::super::CanonicalCAbiResolutionError::LayoutFingerprint(_))
    ));
}

#[test]
fn c_abi_layout_decoder_rejects_zero_alignment() {
    let mut bytes = encode(&layout()).unwrap();
    let offset = bytes
        .windows(3)
        .position(|window| window == [0x03, 0x08, 0x04])
        .unwrap();
    bytes[offset + 1] = 0;
    let error = decode_canonical::<DecodedCanonicalCAbiLayout>(&bytes, DecodeLimits::default())
        .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::IntegerOutOfRange);
}

#[test]
fn c_abi_layout_decoder_rejects_unknown_alignment_and_override() {
    let error =
        decode_canonical::<CLayoutByteAlignment>(b"\x03", DecodeLimits::default()).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });

    let error =
        decode_canonical::<DecodedCLayoutOverride>(b"\xa1\x00\x03", DecodeLimits::default())
            .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });
}

fn layout() -> CanonicalCAbiLayout {
    CanonicalCAbiLayout::new(
        exact_type(),
        8,
        NonZeroU64::new(8).unwrap(),
        CLayoutOverride::Bytes(CLayoutByteAlignment::Bytes8),
        CLayoutOverride::Natural,
        vec![CanonicalCAbiLayoutField::new(
            field(),
            0,
            CanonicalCStorageType::Boolean {
                exact_type: exact_type(),
            },
        )],
    )
}

const fn exact_type() -> PersistentExactTypeId {
    PersistentExactTypeId([1; 32])
}

const fn field() -> PersistentFieldId {
    PersistentFieldId([2; 32])
}

const fn signature() -> CanonicalCAbiSignatureFingerprint {
    CanonicalCAbiSignatureFingerprint([3; 32])
}

const fn referenced_layout() -> CanonicalCAbiLayoutFingerprint {
    CanonicalCAbiLayoutFingerprint([4; 32])
}
