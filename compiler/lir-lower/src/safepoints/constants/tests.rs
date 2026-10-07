use super::*;
use integers::integer_order;

mod arithmetic;
mod storage;
use lir::LirIntegerConstant as C;
use std::cmp::Ordering;

#[test]
fn comparisons_preserve_width_and_signedness_at_the_sign_bit() {
    for (left, right) in [
        (C::Signed8(0x80), C::Signed8(0x7f)),
        (C::Signed16(0x8000), C::Signed16(0x7fff)),
        (C::Signed32(0x8000_0000), C::Signed32(0x7fff_ffff)),
        (C::Signed64(1 << 63), C::Signed64((1 << 63) - 1)),
    ] {
        assert_eq!(
            integer_order(left.kind(), left, right),
            Some(Ordering::Less)
        );
    }
    for (left, right) in [
        (C::Unsigned8(0x80), C::Unsigned8(0x7f)),
        (C::Unsigned16(0x8000), C::Unsigned16(0x7fff)),
        (C::Unsigned32(0x8000_0000), C::Unsigned32(0x7fff_ffff)),
        (C::Unsigned64(1 << 63), C::Unsigned64((1 << 63) - 1)),
    ] {
        assert_eq!(
            integer_order(left.kind(), left, right),
            Some(Ordering::Greater)
        );
    }
}

#[test]
fn local_integer_copies_resolve_the_array_length_guard() {
    let local = lir::LocalId::from_raw(la_arena::RawIdx::from(0));
    let out = lir::TempId::from_raw(la_arena::RawIdx::from(0));
    let mut constants = HashMap::new();
    let store = lir::Instruction::Store {
        local,
        value: lir::Value::IntegerConst(C::Signed64(40)),
    };
    let (key, value) = known_result(&store, &constants).unwrap();
    constants.insert(key, value);
    let comparison = lir::Instruction::IntegerCompare {
        out,
        kind: lir::IntegerKind::SIGNED_64,
        comparison: lir::IntegerComparison::Less,
        lhs: lir::Value::Local(local),
        rhs: lir::Value::IntegerConst(C::Signed64(0)),
    };
    assert_eq!(
        known_result(&comparison, &constants),
        Some((
            LiveValue::Temp(out),
            KnownValue::Scalar(lir::Value::BoolConst(false))
        ))
    );
    let compare_to = lir::Instruction::IntegerCompareTo {
        out,
        operand_kind: lir::IntegerKind::SIGNED_64,
        lhs: lir::Value::Local(local),
        rhs: lir::Value::IntegerConst(C::Signed64(41)),
    };
    assert_eq!(
        known_result(&compare_to, &constants),
        Some((
            LiveValue::Temp(out),
            KnownValue::Scalar(lir::Value::IntegerConst(C::Signed64(u64::MAX)))
        ))
    );
    constants.clear();
    assert_eq!(known_result(&comparison, &constants), None);
}
