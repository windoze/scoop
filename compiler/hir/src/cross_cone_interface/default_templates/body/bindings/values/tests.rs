use scoop_identity::LocalValueSelector;
use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::*;
use crate::CanonicalBooleanV1;

use super::super::test_support::{LocalError, LocalResolver, Resolver, binder, hex, parameter};

#[test]
fn temporary_uses_canonical_local_index_and_round_trips() {
    let expected = DefaultBindingTemporaryV1::new(parameter(0), binder(0));
    let mut locals = LocalResolver::new(vec![LocalValueSelector::This, parameter(0)]);
    let bytes = encode(&expected.index_local(&mut locals).unwrap()).unwrap();

    assert_eq!(hex(&bytes), "a2010102a3000701000200");
    let decoded: DecodedDefaultBindingTemporaryV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(decoded.resolve(&mut Resolver, &mut locals), Ok(expected));
}

#[test]
fn leaf_preserves_mutability_and_round_trips() {
    let expected = DefaultBindingLeafV1::new(parameter(1), binder(2), CanonicalBooleanV1::True);
    let mut locals = LocalResolver::new(vec![LocalValueSelector::This, parameter(0), parameter(1)]);
    let bytes = encode(&expected.index_local(&mut locals).unwrap()).unwrap();

    assert_eq!(hex(&bytes), "a3010202a30007010002020302");
    let decoded: DecodedDefaultBindingLeafV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(decoded.resolve(&mut Resolver, &mut locals), Ok(expected));
}

#[test]
fn typed_binding_values_report_local_index_failures() {
    let temporary = DefaultBindingTemporaryV1::new(parameter(2), binder(0));
    let leaf = DefaultBindingLeafV1::new(parameter(3), binder(0), CanonicalBooleanV1::False);
    let mut locals = LocalResolver::new(vec![parameter(0)]);

    assert_eq!(
        temporary.index_local(&mut locals).unwrap_err(),
        DefaultBindingTemporaryIndexError::Local(LocalError::MissingSelector(parameter(2)))
    );
    assert_eq!(
        leaf.index_local(&mut locals).unwrap_err(),
        DefaultBindingLeafIndexError::Local(LocalError::MissingSelector(parameter(3)))
    );
}

#[test]
fn leaf_decoder_rejects_noncanonical_boolean_tags() {
    let error = decode_canonical::<DecodedDefaultBindingLeafV1>(
        &[
            0xa3, 0x01, 0x00, 0x02, 0xa3, 0x00, 0x07, 0x01, 0x00, 0x02, 0x00, 0x03, 0x03,
        ],
        DecodeLimits::default(),
    )
    .unwrap_err();

    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 3 });
}
