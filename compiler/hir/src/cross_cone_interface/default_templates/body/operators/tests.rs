use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn integer_kind_tags_are_fixed_and_map_to_hir() {
    let values = [
        (DefaultIntegerKindV1::Signed8, crate::IntegerKind::SIGNED_8),
        (
            DefaultIntegerKindV1::Signed16,
            crate::IntegerKind::SIGNED_16,
        ),
        (
            DefaultIntegerKindV1::Signed32,
            crate::IntegerKind::SIGNED_32,
        ),
        (
            DefaultIntegerKindV1::Signed64,
            crate::IntegerKind::SIGNED_64,
        ),
        (
            DefaultIntegerKindV1::Unsigned8,
            crate::IntegerKind::UNSIGNED_8,
        ),
        (
            DefaultIntegerKindV1::Unsigned16,
            crate::IntegerKind::UNSIGNED_16,
        ),
        (
            DefaultIntegerKindV1::Unsigned32,
            crate::IntegerKind::UNSIGNED_32,
        ),
        (
            DefaultIntegerKindV1::Unsigned64,
            crate::IntegerKind::UNSIGNED_64,
        ),
    ];

    for (index, (wire, hir)) in values.into_iter().enumerate() {
        let tag = u8::try_from(index + 1).unwrap();
        assert_eq!(encode(&wire).unwrap(), vec![tag]);
        assert_eq!(
            decode_canonical::<DefaultIntegerKindV1>(&[tag], DecodeLimits::default()).unwrap(),
            wire
        );
        assert_eq!(DefaultIntegerKindV1::from(hir), wire);
        assert_eq!(crate::IntegerKind::from(wire), hir);
    }
}

#[test]
fn operation_tags_are_fixed_and_round_trip() {
    assert_tags(&[
        DefaultNoGcIntegerOperationV1::UnaryPlus,
        DefaultNoGcIntegerOperationV1::UnaryMinus,
        DefaultNoGcIntegerOperationV1::Inc,
        DefaultNoGcIntegerOperationV1::Dec,
        DefaultNoGcIntegerOperationV1::Add,
        DefaultNoGcIntegerOperationV1::Sub,
        DefaultNoGcIntegerOperationV1::Mul,
        DefaultNoGcIntegerOperationV1::CompareTo,
        DefaultNoGcIntegerOperationV1::Equals,
        DefaultNoGcIntegerOperationV1::And,
        DefaultNoGcIntegerOperationV1::Or,
        DefaultNoGcIntegerOperationV1::Xor,
        DefaultNoGcIntegerOperationV1::Inv,
        DefaultNoGcIntegerOperationV1::Shl,
        DefaultNoGcIntegerOperationV1::Shr,
        DefaultNoGcIntegerOperationV1::Ushr,
    ]);
    assert_tags(&[DefaultIntegerDivRemV1::Div, DefaultIntegerDivRemV1::Rem]);
    assert_tags(&[
        DefaultPrimitiveBinaryKindV1::StringConcat,
        DefaultPrimitiveBinaryKindV1::StringCompareTo,
    ]);
    assert_tags(&[DefaultPrimitiveUnaryKindV1::BooleanNot]);
    assert_tags(&[
        DefaultArrayAccessKindV1::ImmutableGet,
        DefaultArrayAccessKindV1::MutableGet,
        DefaultArrayAccessKindV1::MutableSet,
    ]);
    assert_tags(&[
        DefaultForeignCallbackOperationV1::Retain,
        DefaultForeignCallbackOperationV1::Release,
        DefaultForeignCallbackOperationV1::State,
        DefaultForeignCallbackOperationV1::Failure,
    ]);
    assert_tags(&[
        DefaultBinaryOperatorV1::Lt,
        DefaultBinaryOperatorV1::Le,
        DefaultBinaryOperatorV1::Gt,
        DefaultBinaryOperatorV1::Ge,
        DefaultBinaryOperatorV1::RefEq,
        DefaultBinaryOperatorV1::RefNe,
        DefaultBinaryOperatorV1::And,
        DefaultBinaryOperatorV1::Or,
    ]);
    assert_tags(&[DefaultUnaryOperatorV1::Not]);
}

#[test]
fn operation_kinds_map_bidirectionally_to_hir() {
    assert_mappings(&[
        (
            DefaultNoGcIntegerOperationV1::UnaryPlus,
            crate::NoGcIntegerOperation::UnaryPlus,
        ),
        (
            DefaultNoGcIntegerOperationV1::UnaryMinus,
            crate::NoGcIntegerOperation::UnaryMinus,
        ),
        (
            DefaultNoGcIntegerOperationV1::Inc,
            crate::NoGcIntegerOperation::Inc,
        ),
        (
            DefaultNoGcIntegerOperationV1::Dec,
            crate::NoGcIntegerOperation::Dec,
        ),
        (
            DefaultNoGcIntegerOperationV1::Add,
            crate::NoGcIntegerOperation::Add,
        ),
        (
            DefaultNoGcIntegerOperationV1::Sub,
            crate::NoGcIntegerOperation::Sub,
        ),
        (
            DefaultNoGcIntegerOperationV1::Mul,
            crate::NoGcIntegerOperation::Mul,
        ),
        (
            DefaultNoGcIntegerOperationV1::CompareTo,
            crate::NoGcIntegerOperation::CompareTo,
        ),
        (
            DefaultNoGcIntegerOperationV1::Equals,
            crate::NoGcIntegerOperation::Equals,
        ),
        (
            DefaultNoGcIntegerOperationV1::And,
            crate::NoGcIntegerOperation::And,
        ),
        (
            DefaultNoGcIntegerOperationV1::Or,
            crate::NoGcIntegerOperation::Or,
        ),
        (
            DefaultNoGcIntegerOperationV1::Xor,
            crate::NoGcIntegerOperation::Xor,
        ),
        (
            DefaultNoGcIntegerOperationV1::Inv,
            crate::NoGcIntegerOperation::Inv,
        ),
        (
            DefaultNoGcIntegerOperationV1::Shl,
            crate::NoGcIntegerOperation::Shl,
        ),
        (
            DefaultNoGcIntegerOperationV1::Shr,
            crate::NoGcIntegerOperation::Shr,
        ),
        (
            DefaultNoGcIntegerOperationV1::Ushr,
            crate::NoGcIntegerOperation::Ushr,
        ),
    ]);
    assert_mappings(&[
        (DefaultIntegerDivRemV1::Div, crate::IntegerDivRem::Div),
        (DefaultIntegerDivRemV1::Rem, crate::IntegerDivRem::Rem),
    ]);
    assert_mappings(&[
        (
            DefaultPrimitiveBinaryKindV1::StringConcat,
            crate::PrimitiveBinaryKind::StringConcat,
        ),
        (
            DefaultPrimitiveBinaryKindV1::StringCompareTo,
            crate::PrimitiveBinaryKind::StringCompareTo,
        ),
    ]);
    assert_mappings(&[(
        DefaultPrimitiveUnaryKindV1::BooleanNot,
        crate::PrimitiveUnaryKind::BooleanNot,
    )]);
    assert_mappings(&[
        (
            DefaultArrayAccessKindV1::ImmutableGet,
            crate::ArrayAccessKind::ImmutableGet,
        ),
        (
            DefaultArrayAccessKindV1::MutableGet,
            crate::ArrayAccessKind::MutableGet,
        ),
        (
            DefaultArrayAccessKindV1::MutableSet,
            crate::ArrayAccessKind::MutableSet,
        ),
    ]);
    assert_mappings(&[
        (
            DefaultForeignCallbackOperationV1::Retain,
            crate::ForeignCallbackOperation::Retain,
        ),
        (
            DefaultForeignCallbackOperationV1::Release,
            crate::ForeignCallbackOperation::Release,
        ),
        (
            DefaultForeignCallbackOperationV1::State,
            crate::ForeignCallbackOperation::State,
        ),
        (
            DefaultForeignCallbackOperationV1::Failure,
            crate::ForeignCallbackOperation::Failure,
        ),
    ]);
    assert_mappings(&[
        (DefaultBinaryOperatorV1::Lt, crate::BinOp::Lt),
        (DefaultBinaryOperatorV1::Le, crate::BinOp::Le),
        (DefaultBinaryOperatorV1::Gt, crate::BinOp::Gt),
        (DefaultBinaryOperatorV1::Ge, crate::BinOp::Ge),
        (DefaultBinaryOperatorV1::RefEq, crate::BinOp::RefEq),
        (DefaultBinaryOperatorV1::RefNe, crate::BinOp::RefNe),
        (DefaultBinaryOperatorV1::And, crate::BinOp::And),
        (DefaultBinaryOperatorV1::Or, crate::BinOp::Or),
    ]);
    assert_mappings(&[(DefaultUnaryOperatorV1::Not, crate::UnOp::Not)]);
}

#[test]
fn leaf_decoders_reject_unknown_tags() {
    let error = decode_canonical::<DefaultNoGcIntegerOperationV1>(&[17], DecodeLimits::default())
        .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 17 });

    let error =
        decode_canonical::<DefaultUnaryOperatorV1>(&[2], DecodeLimits::default()).unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 2 });
}

fn assert_tags<T>(values: &[T])
where
    T: Copy + Eq + std::fmt::Debug + scoop_wire::WireEncode + scoop_wire::WireDecode,
{
    for (index, value) in values.iter().copied().enumerate() {
        let tag = u8::try_from(index + 1).unwrap();
        assert_eq!(encode(&value).unwrap(), vec![tag]);
        assert_eq!(
            decode_canonical::<T>(&[tag], DecodeLimits::default()).unwrap(),
            value
        );
    }
}

fn assert_mappings<W, H>(values: &[(W, H)])
where
    W: Copy + Eq + std::fmt::Debug + From<H>,
    H: Copy + Eq + std::fmt::Debug + From<W>,
{
    for (wire, hir) in values.iter().copied() {
        assert_eq!(W::from(hir), wire);
        assert_eq!(H::from(wire), hir);
    }
}
