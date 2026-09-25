use scoop_wire::{decode_canonical, encode};

use super::support::*;
use crate::{ExportDefaultCallableTargetV1, ExportDefaultReferenceTargetResolutionError};

#[test]
fn four_field_records_and_six_domain_set_round_trip_without_authority() {
    let f = Fixture::new();
    let set = full_set(&f);
    let bytes = encode(&set).unwrap();
    assert_eq!(&bytes[..2], &[0xa6, 1]);
    let parsed = decoded(&set);
    assert_eq!(encode(&parsed).unwrap(), bytes);
    assert_eq!(parsed.resolve(&mut f.resolver()).unwrap(), set);
    macro_rules! record_wire {
        ($records:expr, $decoded:ty) => {
            let bytes = encode(&$records[0]).unwrap();
            assert_eq!(&bytes[..2], &[0xa4, 1]);
            let decoded: $decoded = decode_canonical(&bytes).unwrap();
            assert_eq!(encode(&decoded).unwrap(), bytes);
            let mut missing_uses = bytes;
            missing_uses[0] = 0xa3;
            assert!(decode_canonical::<$decoded>(&missing_uses).is_err());
        };
    }
    record_wire!(set.callables(), DecodedProtectedDefaultCallableReferenceV1);
    record_wire!(
        set.constructors(),
        DecodedProtectedDefaultConstructorReferenceV1
    );
    record_wire!(set.types(), DecodedProtectedDefaultTypeReferenceV1);
    record_wire!(set.globals(), DecodedProtectedDefaultGlobalReferenceV1);
    record_wire!(
        set.singleton_values(),
        DecodedProtectedDefaultSingletonReferenceV1
    );
    record_wire!(set.fields(), DecodedProtectedDefaultFieldReferenceV1);
    assert!(!set.is_empty());
    let empty = empty_set();
    assert!(empty.is_empty());
    assert_eq!(
        encode(&empty).unwrap(),
        [0xa6, 1, 0x80, 2, 0x80, 3, 0x80, 4, 0x80, 5, 0x80, 6, 0x80]
    );
    assert_eq!(decoded(&empty).resolve(&mut f.resolver()).unwrap(), empty);
}

#[test]
fn malformed_products_field_order_and_unknown_target_tags_are_rejected() {
    for (offset, replacement) in [(0, 0xa5), (1, 2), (2, 0xa0)] {
        let mut bytes = encode(&empty_set()).unwrap();
        bytes[offset] = replacement;
        assert!(decode_canonical::<DecodedProtectedDefaultReferenceSetV1>(&bytes,).is_err());
    }
    let f = Fixture::new();
    let mut bytes = encode(&record(
        &f,
        ExportDefaultCallableTargetV1::Callable(f.callable()),
    ))
    .unwrap();
    assert_eq!(&bytes[..5], &[0xa4, 1, 0xa2, 0, 1]);
    bytes[4] = 9;
    assert!(decode_canonical::<DecodedProtectedDefaultCallableReferenceV1>(&bytes,).is_err());
}

#[test]
fn each_typed_target_must_resolve_in_the_foundation() {
    let f = Fixture::new();
    let kinds = [
        ProtectedDefaultReferenceKindV1::Callable,
        ProtectedDefaultReferenceKindV1::Constructor,
        ProtectedDefaultReferenceKindV1::Type,
        ProtectedDefaultReferenceKindV1::Global,
        ProtectedDefaultReferenceKindV1::Singleton,
        ProtectedDefaultReferenceKindV1::Field,
    ];
    for (set, expected) in singleton_domains(&f).iter().zip(kinds) {
        let error = decoded(set)
            .resolve(&mut Resolver::rejecting())
            .unwrap_err();
        let ProtectedDefaultReferenceSetResolutionError::Record {
            kind,
            index,
            error: ProtectedDefaultReferenceResolutionError::Target(target),
        } = error
        else {
            panic!("{error:?}");
        };
        assert_eq!((kind, index), (expected, 0));
        assert!(matches!(
            (kind, target),
            (
                ProtectedDefaultReferenceKindV1::Callable,
                ExportDefaultReferenceTargetResolutionError::Callable(_)
            ) | (
                ProtectedDefaultReferenceKindV1::Constructor,
                ExportDefaultReferenceTargetResolutionError::Constructor(_)
            ) | (
                ProtectedDefaultReferenceKindV1::Type,
                ExportDefaultReferenceTargetResolutionError::Type(_)
            ) | (
                ProtectedDefaultReferenceKindV1::Global,
                ExportDefaultReferenceTargetResolutionError::Global(_)
            ) | (
                ProtectedDefaultReferenceKindV1::Singleton,
                ExportDefaultReferenceTargetResolutionError::Singleton(_)
            ) | (
                ProtectedDefaultReferenceKindV1::Field,
                ExportDefaultReferenceTargetResolutionError::Field(_)
            )
        ));
    }
}

#[test]
fn metadata_only_use_lists_round_trip_as_data() {
    let f = Fixture::new();
    let mut set = empty_set();
    set.globals = vec![ProtectedDefaultReferenceV1::new(
        f.property,
        f.origin(),
        witness(&f),
        CanonicalProtectedDefaultExpressionUsesV1::try_new(vec![]).unwrap(),
    )];
    let decoded = decoded(&set).resolve(&mut f.resolver()).unwrap();
    assert!(decoded.globals()[0].uses().values().is_empty());
    assert_eq!(decoded, set);
}
