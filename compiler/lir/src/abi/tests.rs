use super::*;

fn non_zero_value(ty: LirType, size: u64, scan: RefScan) -> AbiValue {
    AbiValue::new(
        ty,
        AbiNonZeroLayout::new(size, 8).expect("test layout should be valid"),
        scan,
    )
    .expect("test ABI value should be valid")
}

fn zst(ty: LirType) -> AbiZst {
    AbiZst::new(
        ty,
        AbiZeroSizedLayout::new(1).expect("test layout should be valid"),
    )
    .expect("test ABI ZST should be valid")
}

#[test]
fn interface_parts_preserve_managed_metadata_and_follow_indirect_result() {
    let parts = AbiDirectParts::interface(non_zero_value(
        LirType::Interface,
        16,
        RefScan::References(vec![0]),
    ))
    .unwrap();
    let result = non_zero_value(LirType::Aggregate(vec![LirType::I64; 3]), 24, RefScan::None);
    let signature = ScoopAbiSignature::new(
        vec![
            AbiArgument::ElidedZst(zst(LirType::Aggregate(Vec::new()))),
            AbiArgument::Direct(AbiDirectValue::DirectParts(parts.clone())),
            AbiArgument::Direct(non_zero_value(LirType::I64, 8, RefScan::None).into()),
        ],
        AbiReturn::Indirect(result),
        CallingConvention::Cdecl,
    );
    assert_eq!(signature.physical_parameter_count(), 4);
    assert_eq!(
        signature.argument_location(0),
        Some(AbiArgumentLocation::Elided)
    );
    assert_eq!(
        signature.argument_location(1),
        Some(AbiArgumentLocation::Parts { first: 1, count: 2 })
    );
    assert_eq!(
        signature.argument_location(2),
        Some(AbiArgumentLocation::Parameter(3))
    );
    let physical: Vec<_> = signature.physical_parameters().collect();
    for (index, part) in parts.parts().iter().copied().enumerate() {
        assert_eq!(
            physical[index + 1].origin(),
            AbiPhysicalParameterOrigin::DirectArgumentPart {
                logical_index: 1,
                part
            }
        );
        assert_eq!(physical[index + 1].index(), index + 1);
    }
    assert_eq!(parts.parts()[0].pointer_kind, crate::PointerKind::Managed);
    assert_eq!(parts.parts()[1].pointer_kind, crate::PointerKind::Metadata);
}

#[test]
fn layouts_reject_invalid_size_and_alignment() {
    assert_eq!(
        AbiZeroSizedLayout::new(0),
        Err(AbiLayoutError::ZeroAlignment)
    );
    assert_eq!(
        AbiZeroSizedLayout::new(3),
        Err(AbiLayoutError::AlignmentNotPowerOfTwo(3))
    );
    assert_eq!(AbiNonZeroLayout::new(0, 8), Err(AbiLayoutError::ZeroSize));
    assert_eq!(
        AbiNonZeroLayout::new(16, 6),
        Err(AbiLayoutError::AlignmentNotPowerOfTwo(6))
    );
}

#[test]
fn layouts_expose_only_refined_sizes() {
    let zero = AbiZeroSizedLayout::new(16).expect("zero-sized layout should be valid");
    assert_eq!(zero.size(), 0);
    assert_eq!(zero.alignment().get(), 16);

    let non_zero = AbiNonZeroLayout::new(24, 8).expect("non-zero-sized layout should be valid");
    assert_eq!(non_zero.size().get(), 24);
    assert_eq!(non_zero.alignment().get(), 8);
}

#[test]
fn abi_values_reject_void_storage() {
    let zero_layout = AbiZeroSizedLayout::new(1).expect("test layout should be valid");
    assert_eq!(
        AbiZst::new(LirType::Void, zero_layout),
        Err(AbiValueError::VoidStorageType)
    );

    let non_zero_layout = AbiNonZeroLayout::new(8, 8).expect("test layout should be valid");
    assert_eq!(
        AbiValue::new(LirType::Void, non_zero_layout, RefScan::None),
        Err(AbiValueError::VoidStorageType)
    );
}

#[test]
fn indirect_call_argument_requires_exact_local_storage_type() {
    let mut locals = la_arena::Arena::new();
    let expected = non_zero_value(
        LirType::Aggregate(vec![LirType::I64, LirType::I64]),
        16,
        RefScan::None,
    );
    let local = locals.alloc(super::super::Local::new(
        "argument",
        super::super::LocalStorage::NonZero(expected.clone()),
    ));

    let storage = AbiArgumentStorage::new(local, locals[local].ty(), &expected)
        .expect("exact local storage should be accepted");
    assert_eq!(storage.local(), local);
    assert_eq!(
        AbiCallArgument::Indirect(storage).logical_value(),
        Value::Local(local)
    );

    assert_eq!(
        AbiArgumentStorage::new(local, &LirType::I64, &expected),
        Err(AbiValueError::StorageTypeMismatch {
            expected: expected.storage_type().clone(),
            actual: LirType::I64,
        })
    );
}

#[test]
fn direct_and_elided_call_arguments_preserve_their_logical_value() {
    let direct = Value::IntegerConst(super::super::LirIntegerConstant::Signed64(42));
    let elided = Value::BoolConst(false);

    assert_eq!(AbiCallArgument::Direct(direct).logical_value(), direct);
    assert_eq!(AbiCallArgument::ElidedZst(elided).logical_value(), elided);
}

#[test]
fn signature_computes_physical_parameter_order_without_storing_it() {
    let signature = ScoopAbiSignature::new(
        vec![
            AbiArgument::ElidedZst(zst(LirType::Aggregate(Vec::new()))),
            AbiArgument::Direct(non_zero_value(LirType::I64, 8, RefScan::None).into()),
            AbiArgument::Indirect(non_zero_value(
                LirType::Aggregate(vec![LirType::Ptr(super::super::PointerKind::Managed)]),
                8,
                RefScan::References(vec![0]),
            )),
        ],
        AbiReturn::Indirect(non_zero_value(
            LirType::Aggregate(vec![LirType::I64, LirType::I64]),
            16,
            RefScan::None,
        )),
        CallingConvention::Cdecl,
    );

    assert_eq!(signature.logical_argument_count(), 3);
    assert_eq!(signature.physical_parameter_count(), 3);
    assert_eq!(
        signature.argument_location(0),
        Some(AbiArgumentLocation::Elided)
    );
    assert_eq!(
        signature.argument_location(1),
        Some(AbiArgumentLocation::Parameter(1))
    );
    assert_eq!(
        signature.argument_location(2),
        Some(AbiArgumentLocation::Parameter(2))
    );
    assert_eq!(signature.argument_location(3), None);

    let parameters = signature.physical_parameters().collect::<Vec<_>>();
    assert_eq!(parameters.len(), 3);
    assert_eq!(parameters[0].index(), 0);
    assert_eq!(
        parameters[0].origin(),
        AbiPhysicalParameterOrigin::IndirectReturn
    );
    assert_eq!(
        parameters[1].origin(),
        AbiPhysicalParameterOrigin::DirectArgument { logical_index: 1 }
    );
    assert_eq!(
        parameters[2].origin(),
        AbiPhysicalParameterOrigin::IndirectArgument { logical_index: 2 }
    );
}

#[test]
fn unit_and_zst_results_have_distinct_logical_storage() {
    let unit = AbiReturn::UnitVoid;
    assert_eq!(unit.logical_storage_type(), None);
    assert_eq!(unit.scan(), None);

    let zst = AbiReturn::ElidedZst(zst(LirType::Aggregate(Vec::new())));
    assert_eq!(
        zst.logical_storage_type(),
        Some(&LirType::Aggregate(Vec::new()))
    );
    assert_eq!(zst.scan(), Some(&RefScan::None));
}
