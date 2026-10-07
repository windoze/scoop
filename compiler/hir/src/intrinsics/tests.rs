use super::*;
use scoop_wire::{decode_canonical, encode};

#[test]
fn intrinsic_function_roles_have_a_closed_canonical_wire_identity() {
    let kinds = intrinsic_function_kinds();
    assert_eq!(kinds.len(), INTRINSIC_REGISTRY.len() + 268);
    assert!(kinds.windows(2).all(|pair| pair[0] < pair[1]));

    for kind in kinds {
        let bytes = encode(&kind).unwrap();
        assert_eq!(
            decode_canonical::<IntrinsicFunctionKind>(&bytes).unwrap(),
            kind
        );
    }

    assert_eq!(
        encode(&IntrinsicFunctionKind::GcPinRaw).unwrap(),
        vec![0xa1, 0x00, 0x01]
    );
    assert_eq!(
        encode(&IntrinsicFunctionKind::Integer(
            IntegerIntrinsicKind::Conversion {
                source: IntegerKind::SIGNED_32,
                target_kind: IntegerKind::UNSIGNED_64,
            }
        ))
        .unwrap(),
        vec![
            0xa2, 0x00, 0x0f, 0x01, 0xa3, 0x00, 0x03, 0x01, 0x03, 0x02, 0x08
        ]
    );
    assert_eq!(
        encode(&IntrinsicFunctionKind::Pointer(PointerIntrinsic::AlignOf)).unwrap(),
        vec![0xa2, 0x00, 0x14, 0x01, 0x0b]
    );
}

#[test]
fn intrinsic_function_role_reader_rejects_open_or_invalid_values() {
    for bytes in [
        vec![0xa1, 0x00, 0x15],
        vec![0xa2, 0x00, 0x10, 0x01, 0x02],
        vec![0xa2, 0x00, 0x14, 0x01, 0x0c],
        vec![0xa1, 0x00, 0x17],
        vec![0xa2, 0x00, 0x17, 0x01, 0x04],
        vec![
            0xa2, 0x00, 0x0f, 0x01, 0xa3, 0x00, 0x01, 0x01, 0x05, 0x02, 0x10,
        ],
    ] {
        assert!(decode_canonical::<IntrinsicFunctionKind>(&bytes).is_err());
    }
}

#[test]
fn intrinsic_type_registry_contains_each_integer_kind_once() {
    for kind in IntegerKind::ALL {
        let matches = INTRINSIC_TYPE_REGISTRY
            .iter()
            .filter(|spec| spec.kind == IntrinsicTypeKind::Integer(kind))
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, kind.intrinsic_name());
    }
}

#[test]
fn pointer_intrinsic_families_are_registered_with_distinct_contracts() {
    let pointer = intrinsic_type_spec("core_ptr").expect("Ptr family is registered");
    assert_eq!(pointer.kind, IntrinsicTypeKind::Ptr);
    assert_eq!(pointer.kind.target(), IntrinsicTypeTarget::Struct);
    assert_eq!(
        pointer.kind.parameters(),
        IntrinsicTypeParameters::OneInvariantValue
    );

    let function_pointer =
        intrinsic_type_spec("core_fun_ptr").expect("FunPtr family is registered");
    assert_eq!(function_pointer.kind, IntrinsicTypeKind::FunPtr);
    assert_eq!(function_pointer.kind.target(), IntrinsicTypeTarget::Struct);
    assert_eq!(
        function_pointer.kind.parameters(),
        IntrinsicTypeParameters::OneInvariantUnconstrained
    );
}

#[test]
fn integer_intrinsic_registry_is_total_and_effect_typed() {
    let mut count = 0;
    for kind in IntegerKind::ALL {
        for operation in NoGcIntegerOperation::ALL {
            let name = format!("{}_{}", kind.registry_key(), operation.registry_key());
            if operation.supports(kind) {
                let entry = intrinsic_spec(&name).expect("supported operation is registered");
                assert_eq!(
                    entry.kind(),
                    IntrinsicFunctionKind::Integer(IntegerIntrinsicKind::NoGcOperation {
                        kind,
                        operation,
                    })
                );
                assert!(entry.effects().no_gc);
                count += 1;
            } else {
                assert!(intrinsic_spec(&name).is_none());
            }
        }
        for operation in IntegerDivRem::ALL {
            let name = format!("{}_{}", kind.registry_key(), operation.registry_key());
            let entry = intrinsic_spec(&name).expect("div/rem is registered");
            assert_eq!(
                entry.kind(),
                IntrinsicFunctionKind::Integer(IntegerIntrinsicKind::ManagedOperation {
                    kind,
                    operation,
                })
            );
            assert!(!entry.effects().no_gc);
            count += 1;
        }
        for target_kind in IntegerKind::ALL {
            let name = format!("{}_to_{}", kind.registry_key(), target_kind.registry_key());
            let entry = intrinsic_spec(&name).expect("conversion is registered");
            assert_eq!(
                entry.kind(),
                IntrinsicFunctionKind::Integer(IntegerIntrinsicKind::Conversion {
                    source: kind,
                    target_kind,
                })
            );
            assert!(entry.effects().no_gc);
            count += 1;
        }
    }
    assert_eq!(count, 204);
}
