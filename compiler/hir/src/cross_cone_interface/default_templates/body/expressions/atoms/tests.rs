use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::super::test_support::{Fixture, Resolver, hex};
use super::*;

#[test]
fn string_owner_variants_have_fixed_wire_and_round_trip() {
    let current = DefaultStringOwnerV1::CurrentInstantiation;
    assert_eq!(hex(&encode(&current).unwrap()), "a10001");
    let decoded: DecodedDefaultStringOwnerV1 =
        decode_canonical(&encode(&current).unwrap(), DecodeLimits::default()).unwrap();
    assert_eq!(decoded.resolve(&mut Resolver::rejecting()), Ok(current));

    let fixture = Fixture::new();
    let property = DefaultStringOwnerV1::Property(fixture.property);
    let bytes = encode(&property).unwrap();
    assert_eq!(bytes[2], 2);
    let decoded: DecodedDefaultStringOwnerV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(decoded.resolve(&mut fixture.resolver()), Ok(property));
}

#[test]
fn string_owner_reports_property_resolution_failure() {
    let expected = DefaultStringOwnerV1::Property(Fixture::new().property);
    let decoded: DecodedDefaultStringOwnerV1 =
        decode_canonical(&encode(&expected).unwrap(), DecodeLimits::default()).unwrap();

    assert_eq!(
        decoded.resolve(&mut Resolver::rejecting()),
        Err(DefaultStringOwnerResolutionError::Property(
            super::super::test_support::ResolutionError
        ))
    );
}

#[test]
fn string_owner_decoder_rejects_unknown_tags_and_non_exact_maps() {
    let error = decode_canonical::<DecodedDefaultStringOwnerV1>(
        &[0xa1, 0x00, 0x03],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });

    let error = decode_canonical::<DecodedDefaultStringOwnerV1>(
        &[0xa2, 0x00, 0x01, 0x01, 0x00],
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 1,
            actual: 2,
        }
    );
}
