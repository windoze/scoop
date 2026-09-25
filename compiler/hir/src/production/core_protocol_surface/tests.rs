use scoop_identity::{Effect, SignatureCallableShape, SignatureTypeKey};
use scoop_wire::{decode_canonical, encode};

use super::*;

#[test]
fn compiler_protocol_surface_is_a_closed_eight_field_product() {
    for bytes in [vec![0xa7], vec![0xa9], vec![0xa8, 0x09, 0x00]] {
        assert!(decode_canonical::<DecodedCoreCompilerProtocolSurfaceV1>(&bytes,).is_err());
    }

    let (surface, foundation) = test_support::standalone();
    let bytes = encode(&surface).unwrap();
    assert_eq!(bytes[0], 0xa8);
    let mut retired = bytes.clone();
    retired[0] = 0xa9;
    retired.extend([0x09, 0x80]);
    let error = decode_canonical::<DecodedCoreCompilerProtocolSurfaceV1>(&retired).unwrap_err();
    assert!(matches!(
        error.kind(),
        scoop_wire::WireErrorKind::InvalidLength {
            expected: 8,
            actual: 9
        }
    ));
    let decoded: DecodedCoreCompilerProtocolSurfaceV1 = decode_canonical(&bytes).unwrap();
    assert_eq!(decoded.validate_against(&foundation), Ok(surface));
}

#[test]
fn compiler_protocol_surface_rejects_wrong_callable_kinds() {
    let (mut surface, foundation) = test_support::standalone();
    surface.exception_protocol.0.entries[1] = surface.source_location_protocol.0.entries[1].clone();
    assert_eq!(
        decode(&surface).validate_against(&foundation),
        Err(CoreCompilerProtocolSurfaceValidationError::Relation(
            CoreCompilerProtocolSurfaceRelationError::RoleCallableKindMismatch {
                product: CoreProtocolProductKindV1::Exception,
                index: 1,
            }
        ))
    );
}

#[test]
fn fixed_intrinsic_roles_preserve_signature_and_effect_checks_without_a_total_table() {
    let (surface, foundation) = test_support::standalone();
    let original = callable_entry_ref(surface.source_location_protocol.entries(), 1);
    for signature in [
        SignatureCallableShape::new(
            Effect::Suspend,
            None,
            vec![],
            original.signature().result().clone(),
        ),
        SignatureCallableShape::new(
            Effect::Ordinary,
            None,
            vec![],
            SignatureTypeKey::Nominal(concrete_entry(surface.fundamental_types.entries(), 0)),
        ),
    ] {
        let mut invalid = surface.clone();
        invalid.source_location_protocol.0.entries[1] = CoreProtocolEntryV1::Callable(
            CoreProtocolCallableV1::for_test(original.definition(), signature),
        );
        assert_eq!(
            decode(&invalid).validate_against(&foundation),
            Err(CoreCompilerProtocolSurfaceValidationError::Relation(
                CoreCompilerProtocolSurfaceRelationError::OperationSignatureMismatch(
                    IntrinsicFunctionKind::CurrentSourceLocation
                )
            ))
        );
    }
    let mut invalid = surface.clone();
    invalid.coroutine_protocol.0.entries[12] = invalid.coroutine_protocol.0.entries[11].clone();
    assert_eq!(
        decode(&invalid).validate_against(&foundation),
        Err(CoreCompilerProtocolSurfaceValidationError::Relation(
            CoreCompilerProtocolSurfaceRelationError::DuplicateRoleSubject
        ))
    );
}

#[test]
fn compiler_protocol_surface_rejects_fixed_callable_signature_tampering() {
    let (mut surface, foundation) = test_support::standalone();
    let original = callable_entry_ref(surface.iteration_protocol.entries(), 1).clone();
    surface.iteration_protocol.0.entries[1] =
        CoreProtocolEntryV1::Callable(CoreProtocolCallableV1::for_test(
            original.definition(),
            SignatureCallableShape::new(
                Effect::Ordinary,
                None,
                Vec::new(),
                SignatureTypeKey::Nominal(concrete_entry(surface.fundamental_types.entries(), 0)),
            ),
        ));
    assert_eq!(
        decode(&surface).validate_against(&foundation),
        Err(CoreCompilerProtocolSurfaceValidationError::Relation(
            CoreCompilerProtocolSurfaceRelationError::FixedCallableSignatureMismatch {
                product: CoreProtocolProductKindV1::Iteration,
                index: 1,
            }
        ))
    );

    let (mut surface, foundation) = test_support::standalone();
    surface.exception_protocol.0.entries.swap(11, 12);
    assert_eq!(
        decode(&surface).validate_against(&foundation),
        Err(CoreCompilerProtocolSurfaceValidationError::Relation(
            CoreCompilerProtocolSurfaceRelationError::RoleCallableKindMismatch {
                product: CoreProtocolProductKindV1::Exception,
                index: 11,
            }
        ))
    );
}

#[test]
fn compiler_protocol_surface_replays_owner_variant_and_dispatch_relations() {
    let (mut surface, foundation) = test_support::standalone();
    surface.ffi_protocol.0.entries[4] = surface.source_location_protocol.0.entries[1].clone();
    assert_eq!(
        decode(&surface).validate_against(&foundation),
        Err(
            CoreCompilerProtocolSurfaceValidationError::RoleCallableOwnerMismatch {
                product: CoreProtocolProductKindV1::Ffi,
                index: 4
            }
        )
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
    decode_canonical(&encode(surface).unwrap()).unwrap()
}

#[test]
fn option_protocol_rejects_incomplete_some_and_nonempty_none_shapes() {
    use scoop_identity::{CborIdentityRecord, EnumVariantFieldKey, EnumVariantFieldSelector};

    let (surface, foundation) = test_support::standalone();
    let (_, payload_key) = foundation
        .enum_variant_field_by_bytes(surface.option_some_payload().as_array())
        .unwrap();
    let payload = CborIdentityRecord::from_key(payload_key.clone()).unwrap();
    for (variant, expected) in [(surface.option_some(), 1), (surface.option_none(), 0)] {
        let extra = CborIdentityRecord::from_key(EnumVariantFieldKey::new(
            variant,
            EnumVariantFieldSelector::Positional {
                declaration_index: expected as u32,
            },
        ))
        .unwrap();
        let mut invalid = foundation.clone();
        invalid
            .set_enum_variant_fields(vec![payload.clone(), extra])
            .unwrap();
        assert_eq!(
            decode(&surface).validate_against(&invalid),
            Err(
                CoreCompilerProtocolSurfaceValidationError::OptionVariantFieldCount {
                    variant,
                    expected,
                    actual: expected + 1,
                }
            )
        );
    }
}

mod providers;

mod shared_intrinsics;
