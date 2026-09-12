use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::{DecodedCDataPointee, DecodedCPointerStorage, DecodedCanonicalCStorageType};
use crate::{
    CDataPointee, CPointerStorage, CanonicalCAbiLayoutFingerprint, CanonicalCStorageType,
    IntegerBitWidth, PersistentExactTypeId, PersistentIdMismatch, PersistentIdResolver, Signedness,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

struct Resolver;

impl PersistentIdResolver<PersistentExactTypeId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: crate::DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        id.verify(exact_type())
            .map_err(|_: PersistentIdMismatch<PersistentExactTypeId>| ResolutionError)
    }
}

impl PersistentIdResolver<CanonicalCAbiLayoutFingerprint> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        id: crate::DecodedPersistentId<CanonicalCAbiLayoutFingerprint>,
    ) -> Result<CanonicalCAbiLayoutFingerprint, Self::Error> {
        id.verify(layout())
            .map_err(|_: PersistentIdMismatch<CanonicalCAbiLayoutFingerprint>| ResolutionError)
    }
}

#[test]
fn c_pointer_components_round_trip_and_resolve() {
    let pointees = [
        CDataPointee::OpaqueUnit,
        CDataPointee::ExactObject(exact_type()),
    ];
    for pointee in pointees {
        let decoded = decode_canonical::<DecodedCDataPointee>(
            &encode(&pointee).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), pointee);
    }

    let pointer_storage = [
        CPointerStorage::Direct,
        CPointerStorage::NullableWrapper(exact_type()),
    ];
    for storage in pointer_storage {
        let decoded = decode_canonical::<DecodedCPointerStorage>(
            &encode(&storage).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), storage);
    }
}

#[test]
fn all_c_storage_shapes_round_trip_and_resolve_typed_references() {
    let values = [
        CanonicalCStorageType::Integer {
            exact_type: exact_type(),
            signedness: Signedness::Unsigned,
            bit_width: IntegerBitWidth::Bits32,
        },
        CanonicalCStorageType::Boolean {
            exact_type: exact_type(),
        },
        CanonicalCStorageType::DataPointer {
            exact_type: exact_type(),
            pointee: CDataPointee::ExactObject(exact_type()),
            storage: CPointerStorage::NullableWrapper(exact_type()),
        },
        CanonicalCStorageType::CodePointer {
            exact_type: exact_type(),
            storage: CPointerStorage::Direct,
        },
        CanonicalCStorageType::Struct {
            exact_type: exact_type(),
            layout: layout(),
        },
    ];

    for value in values {
        let decoded = decode_canonical::<DecodedCanonicalCStorageType>(
            &encode(&value).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(decoded.resolve(&mut Resolver).unwrap(), value);
    }
}

#[test]
fn c_storage_resolution_rejects_a_different_same_width_type() {
    let value = CanonicalCStorageType::Boolean {
        exact_type: PersistentExactTypeId([99; 32]),
    };
    let decoded = decode_canonical::<DecodedCanonicalCStorageType>(
        &encode(&value).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(decoded.resolve(&mut Resolver), Err(ResolutionError));
}

#[test]
fn c_storage_decoder_rejects_unknown_tags_and_numeric_kinds() {
    assert_unknown::<DecodedCanonicalCStorageType>(b"\xa1\x00\x06", 6);
    assert_unknown::<DecodedCDataPointee>(b"\xa1\x00\x03", 3);
    assert_unknown::<DecodedCPointerStorage>(b"\xa1\x00\x03", 3);
    assert_unknown::<Signedness>(b"\x03", 3);
    assert_unknown::<IntegerBitWidth>(b"\x18\x18", 24);
}

fn assert_unknown<T: scoop_wire::WireDecode + std::fmt::Debug>(bytes: &[u8], tag: u64) {
    let error = decode_canonical::<T>(bytes, DecodeLimits::default()).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag });
}

const fn exact_type() -> PersistentExactTypeId {
    PersistentExactTypeId([1; 32])
}

const fn layout() -> CanonicalCAbiLayoutFingerprint {
    CanonicalCAbiLayoutFingerprint([3; 32])
}
