use super::super::integer_values::{integer_from_raw, signed_value};
use super::*;
use rustc_apfloat::FloatConvert;
use scoop_hir::{HirFloatConversion, HirIntegerConstant, IntegerKind, IntegerSignedness};

pub(in crate::globals) fn convert_float_constant(
    conversion: HirFloatConversion,
    operand: ConstPropertyValue,
) -> ConstPropertyValue {
    use scoop_identity::FloatConversion as C;
    match (conversion, operand) {
        (C::FromInteger { source, target }, ConstPropertyValue::Integer(value)) => {
            assert_eq!(value.kind(), source);
            match target {
                FloatKind::F32 => pack(target, from_integer::<Single>(value)),
                FloatKind::F64 => pack(target, from_integer::<Double>(value)),
            }
        }
        (C::ToInteger { source, target }, ConstPropertyValue::Float(value)) => {
            assert_eq!(value.kind(), source);
            let bits = u128::from(value.raw_bits());
            let value = match source {
                FloatKind::F32 => to_integer(Single::from_bits(bits), target),
                FloatKind::F64 => to_integer(Double::from_bits(bits), target),
            };
            ConstPropertyValue::Integer(value)
        }
        (C::BetweenFloats { source, target }, ConstPropertyValue::Float(value)) => {
            assert_eq!(value.kind(), source);
            if source == target {
                return ConstPropertyValue::Float(value);
            }
            let bits = u128::from(value.raw_bits());
            let mut loses_info = false;
            match target {
                FloatKind::F32 => {
                    let converted: Single = Double::from_bits(bits)
                        .convert_r(Round::NearestTiesToEven, &mut loses_info)
                        .value;
                    pack(target, converted)
                }
                FloatKind::F64 => {
                    let converted: Double = Single::from_bits(bits)
                        .convert_r(Round::NearestTiesToEven, &mut loses_info)
                        .value;
                    pack(target, converted)
                }
            }
        }
        _ => unreachable!("constant conversion receives its typed source value"),
    }
}

fn from_integer<F: Float>(value: HirIntegerConstant) -> F {
    match value.kind().signedness() {
        IntegerSignedness::Signed => {
            F::from_i128(signed_value(value.kind(), value.raw_bits())).value
        }
        IntegerSignedness::Unsigned => F::from_u128(u128::from(value.raw_bits())).value,
    }
}

fn to_integer<F: Float>(value: F, target: IntegerKind) -> HirIntegerConstant {
    let width = target.width().bits() as usize;
    let raw = match target.signedness() {
        IntegerSignedness::Signed => value.to_i128(width).value as u64,
        IntegerSignedness::Unsigned => value.to_u128(width).value as u64,
    };
    integer_from_raw(target, raw & target.width().raw_mask())
}
