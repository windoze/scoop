use scoop_wire::{WireErrorKind, decode_canonical, encode};

use super::super::test_support::{Fixture, LocalError, definition_path};
use super::*;
use crate::{DefaultExpressionKindV1, DefaultExpressionV1};

#[test]
fn named_callable_reference_round_trips_through_indexed_wire() {
    let fixture = Fixture::new();
    let expected = DefaultCallableReferenceV1::try_new(
        fixture.generated,
        definition_path(),
        DefaultCallableReferenceTargetV1::Named(fixture.callable()),
        fixture.value_type(),
        Vec::new(),
        2,
    )
    .unwrap();

    let bytes = encode(&expected.index_locals(&mut fixture.locals()).unwrap()).unwrap();
    let decoded: DecodedDefaultCallableReferenceV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    assert_eq!(
        decoded.resolve(&mut fixture.resolver(), &mut fixture.locals()),
        Ok(expected)
    );
}

#[test]
fn bound_target_index_error_keeps_target_tag() {
    let fixture = Fixture::new();
    let missing_receiver = DefaultExpressionV1::try_new(
        DefaultExpressionKindV1::Local(scoop_identity::LocalValueSelector::Parameter {
            declaration_index: 1,
        }),
        fixture.value_type(),
        fixture.origin(),
        scoop_identity::EvaluationOrigin::at_definition(fixture.origin().origin()),
    )
    .unwrap();
    let reference = DefaultCallableReferenceV1::try_new(
        fixture.generated,
        definition_path(),
        DefaultCallableReferenceTargetV1::BoundExtension {
            receiver: Box::new(missing_receiver),
            callee: fixture.callable(),
        },
        fixture.value_type(),
        Vec::new(),
        0,
    )
    .unwrap();

    assert_eq!(
        reference.index_locals(&mut fixture.locals()).unwrap_err(),
        DefaultCallableReferenceIndexError::TargetReceiver {
            target_tag: 4,
            error: Box::new(crate::DefaultExpressionIndexError::Local(LocalError)),
        }
    );
}

#[test]
fn callable_reference_target_decoder_rejects_unknown_and_non_exact_sums() {
    let error = decode_canonical::<DecodedDefaultCallableReferenceTargetV1>(&[0xa1, 0x00, 0x05])
        .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 5 });

    let error = decode_canonical::<DecodedDefaultCallableReferenceTargetV1>(&[0xa1, 0x00, 0x01])
        .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 2,
            actual: 1,
        }
    );
}
