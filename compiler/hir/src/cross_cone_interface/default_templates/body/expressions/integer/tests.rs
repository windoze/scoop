use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::super::test_support::{Fixture, ResolutionError, Resolver};
use super::*;

#[test]
fn integer_operation_variants_have_fixed_tags_and_round_trip() {
    let fixture = Fixture::new();
    let cases = [
        DefaultIntegerOperationV1::NoGc {
            kind: DefaultIntegerKindV1::Signed32,
            operation: DefaultNoGcIntegerOperationV1::Add,
            target: fixture.callable(),
        },
        DefaultIntegerOperationV1::Managed {
            kind: DefaultIntegerKindV1::Unsigned64,
            operation: DefaultIntegerDivRemV1::Rem,
            target: fixture.callable(),
        },
    ];

    for (expected_tag, expected) in [1, 2].into_iter().zip(cases) {
        let bytes = encode(&expected).unwrap();
        assert_eq!(bytes[2], expected_tag);
        let decoded: DecodedDefaultIntegerOperationV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(decoded.resolve(&mut fixture.resolver()), Ok(expected));
    }
}

#[test]
fn integer_operation_distinguishes_target_resolution_failures() {
    let fixture = Fixture::new();
    let expected = DefaultIntegerOperationV1::Managed {
        kind: DefaultIntegerKindV1::Signed64,
        operation: DefaultIntegerDivRemV1::Div,
        target: fixture.callable(),
    };
    let decoded: DecodedDefaultIntegerOperationV1 =
        decode_canonical(&encode(&expected).unwrap(), DecodeLimits::default()).unwrap();

    assert_eq!(
        decoded.resolve(&mut Resolver::rejecting()),
        Err(DefaultIntegerOperationResolutionError::ManagedTarget(
            DefaultCallableRefResolutionError::Declaration(ResolutionError)
        ))
    );
}

#[test]
fn integer_operation_decoder_rejects_unknown_tags_and_non_exact_maps() {
    let error = decode_canonical::<DecodedDefaultIntegerOperationV1>(
        &[0xa4, 0x00, 0x03, 0x01, 0x03, 0x02, 0x01, 0x03, 0xa0],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });

    let error = decode_canonical::<DecodedDefaultIntegerOperationV1>(
        &[0xa1, 0x00, 0x01],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 4,
            actual: 1,
        }
    );
}
