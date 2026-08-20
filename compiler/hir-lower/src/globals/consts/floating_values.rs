//! Target-precision constant arithmetic shared by const and static initializers.

use rustc_apfloat::{
    Float, Round,
    ieee::{Double, Single},
};
use scoop_hir::{
    ConstPropertyValue, FloatBinaryOperator as B, FloatKind, FloatUnaryOperator as U,
    HirFloatConstant,
};

mod conversions;
pub(in crate::globals) use conversions::convert_float_constant;

pub(in crate::globals) fn float_binary_operator(operator: scoop_ast::BinOp) -> Option<B> {
    use scoop_ast::BinOp as A;
    Some(match operator {
        A::Add => B::Add,
        A::Sub => B::Subtract,
        A::Mul => B::Multiply,
        A::Div => B::Divide,
        A::Rem => B::Remainder,
        A::Eq => B::Equal,
        A::Ne => B::NotEqual,
        A::Lt => B::Less,
        A::Le => B::LessEqual,
        A::Gt => B::Greater,
        A::Ge => B::GreaterEqual,
        _ => return None,
    })
}

pub(in crate::globals) fn evaluate_float_unary(
    operation: U,
    operand: HirFloatConstant,
) -> ConstPropertyValue {
    match operation {
        U::Plus => ConstPropertyValue::Float(operand),
        U::Negate => ConstPropertyValue::Float(operand.negate()),
        U::Increment | U::Decrement => {
            let one = match operand.kind() {
                FloatKind::F32 => HirFloatConstant::F32(0x3f80_0000),
                FloatKind::F64 => HirFloatConstant::F64(0x3ff0_0000_0000_0000),
            };
            evaluate_float_binary(
                if operation == U::Increment {
                    B::Add
                } else {
                    B::Subtract
                },
                operand,
                one,
            )
        }
        U::IsNaN | U::IsInfinite | U::IsFinite => {
            let classify = |nan: bool, infinite: bool| match operation {
                U::IsNaN => nan,
                U::IsInfinite => infinite,
                U::IsFinite => !nan && !infinite,
                _ => unreachable!("classification has one of three predicates"),
            };
            let bits = u128::from(operand.raw_bits());
            let value = match operand.kind() {
                FloatKind::F32 => {
                    let value = Single::from_bits(bits);
                    classify(value.is_nan(), value.is_infinite())
                }
                FloatKind::F64 => {
                    let value = Double::from_bits(bits);
                    classify(value.is_nan(), value.is_infinite())
                }
            };
            ConstPropertyValue::Boolean(value)
        }
    }
}

pub(in crate::globals) fn evaluate_float_binary(
    operation: B,
    lhs: HirFloatConstant,
    rhs: HirFloatConstant,
) -> ConstPropertyValue {
    assert_eq!(
        lhs.kind(),
        rhs.kind(),
        "typed floating operands have the same format"
    );
    if operation == B::TotalOrder {
        return ConstPropertyValue::Boolean(order_key(lhs) <= order_key(rhs));
    }
    let left = u128::from(lhs.raw_bits());
    let right = u128::from(rhs.raw_bits());
    match lhs.kind() {
        FloatKind::F32 => binary(
            operation,
            Single::from_bits(left),
            Single::from_bits(right),
            FloatKind::F32,
        ),
        FloatKind::F64 => binary(
            operation,
            Double::from_bits(left),
            Double::from_bits(right),
            FloatKind::F64,
        ),
    }
}

fn binary<F: Float>(operation: B, lhs: F, rhs: F, kind: FloatKind) -> ConstPropertyValue {
    let round = Round::NearestTiesToEven;
    let value = match operation {
        B::Add => lhs.add_r(rhs, round).value,
        B::Subtract => lhs.sub_r(rhs, round).value,
        B::Multiply => lhs.mul_r(rhs, round).value,
        B::Divide => lhs.div_r(rhs, round).value,
        B::Remainder => lhs.c_fmod(rhs).value,
        B::Equal => return ConstPropertyValue::Boolean(lhs == rhs),
        B::NotEqual => return ConstPropertyValue::Boolean(lhs != rhs),
        B::Less => return ConstPropertyValue::Boolean(lhs < rhs),
        B::LessEqual => return ConstPropertyValue::Boolean(lhs <= rhs),
        B::Greater => return ConstPropertyValue::Boolean(lhs > rhs),
        B::GreaterEqual => return ConstPropertyValue::Boolean(lhs >= rhs),
        B::TotalOrder => unreachable!("totalOrder uses the original bits"),
    };
    // Arithmetic NaNs have the language's fixed positive quiet representation.
    pack(kind, if value.is_nan() { F::NAN } else { value })
}

fn pack<F: Float>(kind: FloatKind, value: F) -> ConstPropertyValue {
    ConstPropertyValue::Float(match kind {
        FloatKind::F32 => HirFloatConstant::F32(value.to_bits() as u32),
        FloatKind::F64 => HirFloatConstant::F64(value.to_bits() as u64),
    })
}

fn order_key(value: HirFloatConstant) -> u64 {
    let sign = 1_u64 << (value.kind().bits() - 1);
    let raw = value.raw_bits();
    if raw & sign == 0 {
        raw ^ sign
    } else {
        !raw & match value.kind() {
            FloatKind::F32 => u64::from(u32::MAX),
            FloatKind::F64 => u64::MAX,
        }
    }
}
