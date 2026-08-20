use super::*;

fn out() -> lir::TempId {
    lir::TempId::from_raw(la_arena::RawIdx::from(0))
}

fn evaluate(instruction: lir::Instruction) -> Option<C> {
    let (_, value) = integers::result(&instruction, Some)?;
    let lir::Value::IntegerConst(value) = value else {
        panic!("expected an integer result");
    };
    Some(value)
}

#[test]
fn arithmetic_wraps_at_each_declared_width() {
    for (maximum_bits, one, zero) in [
        (C::Signed8(u8::MAX), C::Signed8(1), C::Signed8(0)),
        (C::Signed16(u16::MAX), C::Signed16(1), C::Signed16(0)),
        (C::Signed32(u32::MAX), C::Signed32(1), C::Signed32(0)),
        (C::Signed64(u64::MAX), C::Signed64(1), C::Signed64(0)),
        (C::Unsigned8(u8::MAX), C::Unsigned8(1), C::Unsigned8(0)),
        (C::Unsigned16(u16::MAX), C::Unsigned16(1), C::Unsigned16(0)),
        (C::Unsigned32(u32::MAX), C::Unsigned32(1), C::Unsigned32(0)),
        (C::Unsigned64(u64::MAX), C::Unsigned64(1), C::Unsigned64(0)),
    ] {
        for (operation, left, right, expected) in [
            (lir::IntegerBinaryOperation::Add, maximum_bits, one, zero),
            (
                lir::IntegerBinaryOperation::Subtract,
                zero,
                one,
                maximum_bits,
            ),
            (
                lir::IntegerBinaryOperation::Multiply,
                maximum_bits,
                maximum_bits,
                one,
            ),
        ] {
            assert_eq!(
                evaluate(lir::Instruction::IntegerBinary {
                    out: out(),
                    kind: left.kind(),
                    operation,
                    lhs: lir::Value::IntegerConst(left),
                    rhs: lir::Value::IntegerConst(right),
                }),
                Some(expected)
            );
        }
    }
}

#[test]
fn conversions_use_source_signedness_and_target_width() {
    for (source, expected) in [
        (C::Signed8(0xff), C::Unsigned64(u64::MAX)),
        (C::Unsigned8(0xff), C::Signed64(255)),
        (C::Signed64(0x1234), C::Unsigned8(0x34)),
        (C::Signed16(0x8000), C::Signed32(0xffff_8000)),
    ] {
        assert_eq!(
            evaluate(lir::Instruction::IntegerConvert {
                out: out(),
                source_kind: source.kind(),
                target_kind: expected.kind(),
                operand: lir::Value::IntegerConst(source),
            }),
            Some(expected)
        );
    }
}

#[test]
fn division_preserves_truncation_and_leaves_exceptional_operands_unfolded() {
    for (left, right, operation, expected) in [
        (
            C::Signed8(0xf9),
            C::Signed8(3),
            lir::IntegerDivRemOperation::Divide,
            Some(C::Signed8(0xfe)),
        ),
        (
            C::Signed8(0xf9),
            C::Signed8(3),
            lir::IntegerDivRemOperation::Remainder,
            Some(C::Signed8(0xff)),
        ),
        (
            C::Signed8(0x80),
            C::Signed8(0xff),
            lir::IntegerDivRemOperation::Divide,
            None,
        ),
        (
            C::Signed64(1 << 63),
            C::Signed64(u64::MAX),
            lir::IntegerDivRemOperation::Remainder,
            None,
        ),
        (
            C::Unsigned32(7),
            C::Unsigned32(0),
            lir::IntegerDivRemOperation::Divide,
            None,
        ),
    ] {
        assert_eq!(
            evaluate(lir::Instruction::SafeIntegerDivRem {
                out: out(),
                kind: left.kind(),
                operation,
                lhs: lir::Value::IntegerConst(left),
                rhs: lir::Value::IntegerConst(right),
            }),
            expected
        );
    }
}

#[test]
fn shifts_distinguish_sign_extension_and_reject_an_unnormalized_count() {
    for (operation, count, expected) in [
        (
            lir::IntegerShiftOperation::ArithmeticRight,
            7,
            Some(C::Signed8(0xff)),
        ),
        (
            lir::IntegerShiftOperation::LogicalRight,
            7,
            Some(C::Signed8(1)),
        ),
        (lir::IntegerShiftOperation::Left, 7, Some(C::Signed8(0x80))),
        (lir::IntegerShiftOperation::Left, 8, None),
    ] {
        assert_eq!(
            evaluate(lir::Instruction::IntegerShift {
                out: out(),
                kind: lir::IntegerKind::SIGNED_8,
                operation,
                value: lir::Value::IntegerConst(C::Signed8(0xff)),
                normalized_count: lir::Value::IntegerConst(C::Signed8(count)),
            }),
            expected
        );
    }
}
