//! Target-precision constant arithmetic shared by const and static initializers.

use rustc_apfloat::{
    Float, Round,
    ieee::{Double, Single},
};
use scoop_hir::{
    ConstPropertyValue, FloatBinaryOperator as B, FloatKind, FloatUnaryOperator as U,
    HirFloatConstant, HirIntegerConstant,
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
        B::Equal
        | B::NotEqual
        | B::Less
        | B::LessEqual
        | B::Greater
        | B::GreaterEqual
        | B::CompareTo => {
            let order = match (lhs.is_nan(), rhs.is_nan()) {
                (true, true) => std::cmp::Ordering::Equal,
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                (false, false) => lhs.partial_cmp(&rhs).expect("non-NaN floats are ordered"),
            };
            if operation == B::CompareTo {
                return ConstPropertyValue::Integer(HirIntegerConstant::Signed64(
                    order as i64 as u64,
                ));
            }
            return ConstPropertyValue::Boolean(match operation {
                B::Equal => order.is_eq(),
                B::NotEqual => order.is_ne(),
                B::Less => order.is_lt(),
                B::LessEqual => order.is_le(),
                B::Greater => order.is_gt(),
                B::GreaterEqual => order.is_ge(),
                _ => unreachable!("comparison arm contains only predicates"),
            });
        }
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
