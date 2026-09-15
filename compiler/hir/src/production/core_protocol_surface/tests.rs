use scoop_identity::{Effect, SignatureCallableShape};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::*;

#[test]
fn compiler_protocol_surface_is_a_closed_nine_field_product() {
    for bytes in [vec![0xa8], vec![0xaa], vec![0xa9, 0x0a, 0x00]] {
        assert!(
            decode_canonical::<DecodedCoreCompilerProtocolSurfaceV1>(
                &bytes,
                DecodeLimits::default(),
            )
            .is_err()
        );
    }

    let (surface, foundation) = test_support::standalone();
    let bytes = encode(&surface).unwrap();
    let decoded: DecodedCoreCompilerProtocolSurfaceV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert_eq!(decoded.validate_against(&foundation), Ok(surface));
}

#[test]
fn compiler_protocol_surface_rejects_wrong_callable_kinds_and_missing_operations() {
    let (mut surface, foundation) = test_support::standalone();
    surface.exception_protocol.0.entries[1] = CoreProtocolEntryV1::Callable(
        surface.compiler_operation_protocol.operations[0]
            .callable
            .clone(),
    );
    assert_eq!(
        decode(&surface).validate_against(&foundation),
        Err(CoreCompilerProtocolSurfaceValidationError::Relation(
            CoreCompilerProtocolSurfaceRelationError::RoleCallableKindMismatch {
                product: CoreProtocolProductKindV1::Exception,
                index: 1,
            }
        ))
    );

    let (mut surface, foundation) = test_support::standalone();
    surface.compiler_operation_protocol.operations.pop();
    assert_eq!(
        decode(&surface).validate_against(&foundation),
        Err(CoreCompilerProtocolSurfaceValidationError::Relation(
            CoreCompilerProtocolSurfaceRelationError::OperationCoverage {
                expected: intrinsic_function_kinds().len(),
                actual: intrinsic_function_kinds().len() - 1,
            }
        ))
    );
}

#[test]
fn compiler_protocol_surface_rejects_operation_effect_and_repetition_tampering() {
    let (mut surface, foundation) = test_support::standalone();
    let kind = surface.compiler_operation_protocol.operations[0].kind;
    {
        let operation = &mut surface.compiler_operation_protocol.operations[0];
        let original = operation.callable.clone();
        operation.callable = CoreProtocolCallableV1::for_test(
            original.definition(),
            SignatureCallableShape::new(
                Effect::Suspend,
                None,
                original.signature().parameters().to_vec(),
                original.signature().result().clone(),
            ),
        );
    }
    assert_eq!(
        decode(&surface).validate_against(&foundation),
        Err(CoreCompilerProtocolSurfaceValidationError::Relation(
            CoreCompilerProtocolSurfaceRelationError::OperationEffectMismatch(kind)
        ))
    );

    let (mut surface, foundation) = test_support::standalone();
    let duplicate = surface.compiler_operation_protocol.operations[0]
        .callable
        .clone();
    let definition = duplicate.definition();
    surface.compiler_operation_protocol.operations[1].callable = duplicate;
    assert_eq!(
        decode(&surface).validate_against(&foundation),
        Err(CoreCompilerProtocolSurfaceValidationError::Relation(
            CoreCompilerProtocolSurfaceRelationError::DuplicateOperationCallable(definition)
        ))
    );
}

#[test]
fn compiler_protocol_surface_rejects_fixed_and_total_operation_disagreement() {
    let (mut surface, foundation) = test_support::standalone();
    let replacement = surface
        .compiler_operation_protocol
        .operations
        .iter()
        .find(|operation| operation.kind == IntrinsicFunctionKind::GcCollect)
        .unwrap()
        .callable
        .clone();
    surface.ffi_protocol.0.entries[4] = CoreProtocolEntryV1::Callable(replacement);
    assert_eq!(
        decode(&surface).validate_against(&foundation),
        Err(CoreCompilerProtocolSurfaceValidationError::Relation(
            CoreCompilerProtocolSurfaceRelationError::RepeatedOperationMismatch(
                IntrinsicFunctionKind::Pointer(crate::PointerIntrinsic::ToULong)
            )
        ))
    );
}

#[test]
fn compiler_protocol_surface_replays_owner_variant_and_dispatch_relations() {
    let (mut surface, foundation) = test_support::standalone();
    let int8 = IntrinsicFunctionKind::Integer(crate::IntegerIntrinsicKind::NoGcOperation {
        kind: crate::IntegerKind::SIGNED_8,
        operation: crate::NoGcIntegerOperation::UnaryPlus,
    });
    let int16 = IntrinsicFunctionKind::Integer(crate::IntegerIntrinsicKind::NoGcOperation {
        kind: crate::IntegerKind::SIGNED_16,
        operation: crate::NoGcIntegerOperation::UnaryPlus,
    });
    let int8_index = surface
        .compiler_operation_protocol
        .operations
        .iter()
        .position(|operation| operation.kind == int8)
        .unwrap();
    let int16_index = surface
        .compiler_operation_protocol
        .operations
        .iter()
        .position(|operation| operation.kind == int16)
        .unwrap();
    surface
        .compiler_operation_protocol
        .operations
        .swap(int8_index, int16_index);
    surface.compiler_operation_protocol.operations[int8_index].kind = int8;
    surface.compiler_operation_protocol.operations[int16_index].kind = int16;
    assert_eq!(
        decode(&surface).validate_against(&foundation),
        Err(CoreCompilerProtocolSurfaceValidationError::OperationOwnerMismatch(int8))
    );

    let (mut surface, foundation) = test_support::standalone();
    surface.option_protocol.0.entries[1] = surface.foreign_callback_protocol.0.entries[2].clone();
    assert_eq!(
        decode(&surface).validate_against(&foundation),
        Err(CoreCompilerProtocolSurfaceValidationError::OptionOwnerMismatch)
    );

    let (mut surface, foundation) = test_support::standalone();
    surface.iteration_protocol.0.entries[2] = surface.coroutine_protocol.0.entries[2].clone();
    assert_eq!(
        decode(&surface).validate_against(&foundation),
        Err(
            CoreCompilerProtocolSurfaceValidationError::InterfaceDispatchMismatch {
                product: CoreProtocolProductKindV1::Iteration,
                callable_index: 1,
                slot_index: 2,
            }
        )
    );
}

fn decode(surface: &CoreCompilerProtocolSurfaceV1) -> DecodedCoreCompilerProtocolSurfaceV1 {
    decode_canonical(&encode(surface).unwrap(), DecodeLimits::default()).unwrap()
}
