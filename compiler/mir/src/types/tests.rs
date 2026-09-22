use super::*;

#[test]
fn all_source_integer_kinds_have_exact_names_and_widths() {
    let expected = [
        (
            IntegerKind::SIGNED_8,
            IntegerSignedness::Signed,
            IntegerWidth::W8,
            "Int8",
        ),
        (
            IntegerKind::SIGNED_16,
            IntegerSignedness::Signed,
            IntegerWidth::W16,
            "Int16",
        ),
        (
            IntegerKind::SIGNED_32,
            IntegerSignedness::Signed,
            IntegerWidth::W32,
            "Int",
        ),
        (
            IntegerKind::SIGNED_64,
            IntegerSignedness::Signed,
            IntegerWidth::W64,
            "Long",
        ),
        (
            IntegerKind::UNSIGNED_8,
            IntegerSignedness::Unsigned,
            IntegerWidth::W8,
            "UInt8",
        ),
        (
            IntegerKind::UNSIGNED_16,
            IntegerSignedness::Unsigned,
            IntegerWidth::W16,
            "UInt16",
        ),
        (
            IntegerKind::UNSIGNED_32,
            IntegerSignedness::Unsigned,
            IntegerWidth::W32,
            "UInt",
        ),
        (
            IntegerKind::UNSIGNED_64,
            IntegerSignedness::Unsigned,
            IntegerWidth::W64,
            "ULong",
        ),
    ];

    assert_eq!(IntegerKind::ALL.len(), expected.len());
    for (index, (kind, signedness, width, name)) in expected.into_iter().enumerate() {
        assert_eq!(IntegerKind::ALL[index], kind);
        assert_eq!(kind.signedness(), signedness);
        assert_eq!(kind.width(), width);
        assert_eq!(kind.canonical_name(), name);
        assert_eq!(width.bytes() * 8, width.bits());
    }
}

#[test]
fn integer_constants_derive_every_property_from_the_exact_variant() {
    let constants = [
        (
            MirIntegerConstant::Signed8(0x80),
            IntegerKind::SIGNED_8,
            -128,
        ),
        (
            MirIntegerConstant::Signed16(0x8000),
            IntegerKind::SIGNED_16,
            -32_768,
        ),
        (
            MirIntegerConstant::Signed32(0x8000_0000),
            IntegerKind::SIGNED_32,
            -2_147_483_648,
        ),
        (
            MirIntegerConstant::Signed64(0x8000_0000_0000_0000),
            IntegerKind::SIGNED_64,
            -9_223_372_036_854_775_808,
        ),
        (
            MirIntegerConstant::Unsigned8(u8::MAX),
            IntegerKind::UNSIGNED_8,
            u8::MAX as i128,
        ),
        (
            MirIntegerConstant::Unsigned16(u16::MAX),
            IntegerKind::UNSIGNED_16,
            u16::MAX as i128,
        ),
        (
            MirIntegerConstant::Unsigned32(u32::MAX),
            IntegerKind::UNSIGNED_32,
            u32::MAX as i128,
        ),
        (
            MirIntegerConstant::Unsigned64(u64::MAX),
            IntegerKind::UNSIGNED_64,
            u64::MAX as i128,
        ),
    ];

    for (constant, kind, mathematical_value) in constants {
        assert_eq!(constant.kind(), kind);
        assert_eq!(constant.signedness(), kind.signedness());
        assert_eq!(constant.width(), kind.width());
        assert_eq!(constant.mathematical_value(), mathematical_value);
        assert_eq!(
            MirIntegerConstant::from_raw_bits(kind, constant.raw_bits()),
            Some(constant)
        );
        assert_eq!(Expr::integer(constant).ty, Type::Integer(kind));
    }

    assert_eq!(
        MirIntegerConstant::from_raw_bits(IntegerKind::SIGNED_8, 0x100),
        None
    );
    assert_eq!(
        MirIntegerConstant::from_raw_bits(IntegerKind::UNSIGNED_32, 1_u64 << 32),
        None
    );
}

#[test]
fn static_and_annotation_integer_zero_remain_exactly_typed() {
    let zero = MirIntegerConstant::Unsigned16(0);
    let encoded = MirStaticInitialState::EncodedStaticValue {
        payload: MirConstantImage::Integer(zero),
    };
    assert_ne!(encoded, MirStaticInitialState::ZeroedForRuntimeUnit);
    assert_eq!(
        MirAnnotationValue::Integer(zero),
        MirAnnotationValue::Integer(MirIntegerConstant::Unsigned16(0))
    );
    assert_ne!(
        MirAnnotationValue::Integer(zero),
        MirAnnotationValue::Integer(MirIntegerConstant::Signed16(0))
    );
}

#[test]
fn c_layout_annotation_contract_has_only_qualified_alignments() {
    let values = [
        MirCLayoutValue::Natural,
        MirCLayoutValue::A1,
        MirCLayoutValue::A2,
        MirCLayoutValue::A4,
        MirCLayoutValue::A8,
        MirCLayoutValue::A16,
    ];
    assert_eq!(
        values.map(MirCLayoutValue::bytes),
        [None, Some(1), Some(2), Some(4), Some(8), Some(16)]
    );
}

#[test]
fn suspend_state_ids_are_nonzero_by_construction() {
    assert_eq!(CoroutineSuspendStateId::new(0), None);
    assert_eq!(
        CoroutineSuspendStateId::new(1).map(CoroutineSuspendStateId::get),
        Some(1)
    );
    assert_eq!(
        CoroutineSuspendStateId::new(u32::MAX).map(CoroutineSuspendStateId::get),
        Some(u32::MAX)
    );
}

#[test]
fn coroutine_frame_state_encodings_do_not_overlap() {
    let first = CoroutineSuspendStateId::new(1).expect("one is nonzero");
    let last = CoroutineSuspendStateId::new(u32::MAX).expect("u32::MAX is nonzero");
    let initial = MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Initial).raw_bits();
    let running = MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Running).raw_bits();
    let completed =
        MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Completed).raw_bits();
    let first_suspended =
        MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Suspended(first)).raw_bits();
    let last_suspended =
        MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Suspended(last)).raw_bits();
    let first_failure =
        MachineScalarValue::CoroutineFrameState(CoroutineFrameState::ResumeFailure(first))
            .raw_bits();
    let last_failure =
        MachineScalarValue::CoroutineFrameState(CoroutineFrameState::ResumeFailure(last))
            .raw_bits();

    assert_eq!(initial, 0);
    assert_eq!(first_suspended, 1);
    assert_eq!(last_suspended, u64::from(u32::MAX));
    assert!(last_suspended < last_failure);
    assert!(last_failure <= first_failure);
    assert!(first_failure < completed);
    assert!(completed < running);
}

#[test]
fn machine_scalar_values_have_one_total_semantic_kind() {
    let site = CoroutineSuspendStateId::new(1).expect("one is nonzero");
    let cases = [
        (MachineScalarValue::EnumTag(7), MachineScalarKind::EnumTag),
        (
            MachineScalarValue::InitializationOutcome(InitializationOutcome::Ready),
            MachineScalarKind::InitializationOutcome,
        ),
        (
            MachineScalarValue::CoroutineFrameState(CoroutineFrameState::Suspended(site)),
            MachineScalarKind::CoroutineFrameState,
        ),
        (
            MachineScalarValue::CoroutineAdapterState(CoroutineAdapterState::Waiting),
            MachineScalarKind::CoroutineAdapterState,
        ),
        (
            MachineScalarValue::ForeignCallbackStatus(ForeignCallbackStatus::Returned),
            MachineScalarKind::ForeignCallbackStatus,
        ),
        (
            MachineScalarValue::PointerElementOffset(9),
            MachineScalarKind::PointerElementOffset,
        ),
    ];

    for (value, expected) in cases {
        assert_eq!(value.kind(), expected);
        assert_eq!(
            Expr::machine_scalar(value).ty,
            Type::MachineScalar(expected)
        );
    }
    assert_eq!(
        Expr::enum_tag(Expr::unit()).ty,
        Type::MachineScalar(MachineScalarKind::EnumTag)
    );
}

#[test]
#[should_panic(expected = "machine scalar equality operands have the same semantic kind")]
fn machine_scalar_equality_rejects_mismatched_kinds() {
    let _ = Expr::machine_eq(
        Expr::machine_scalar(MachineScalarValue::EnumTag(0)),
        MachineScalarValue::ForeignCallbackStatus(ForeignCallbackStatus::Returned),
    );
}
