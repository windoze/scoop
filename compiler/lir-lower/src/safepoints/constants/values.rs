use super::*;

pub(super) fn known_result(
    instruction: &lir::Instruction,
    constants: &HashMap<LiveValue, KnownValue>,
) -> Option<(LiveValue, KnownValue)> {
    let scalar = |value| scalar_value(value, constants);
    let (out, value) = match *instruction {
        lir::Instruction::Store { local, value } => {
            return Some((LiveValue::Local(local), known_value(value, constants)?));
        }
        lir::Instruction::EnumWrap { out, variant, .. } => {
            return Some((LiveValue::Temp(out), KnownValue::Variant(variant)));
        }
        lir::Instruction::EnumTag {
            out,
            enum_id,
            operand,
        } => {
            let KnownValue::Variant(variant) = known_value(operand, constants)? else {
                return None;
            };
            if variant.definition() != enum_id {
                return None;
            }
            (
                out,
                lir::Value::MachineScalar(lir::MachineScalarValue::EnumTag(variant.index())),
            )
        }
        lir::Instruction::VariantTest {
            out,
            operand,
            variant,
        } => {
            let KnownValue::Variant(actual) = known_value(operand, constants)? else {
                return None;
            };
            if actual.definition() != variant.definition() {
                return None;
            }
            (out, lir::Value::BoolConst(actual == variant))
        }
        lir::Instruction::UnaryOp {
            out,
            op: lir::UnOp::Not,
            operand,
        } => (
            out,
            lir::Value::BoolConst(!boolean_value(operand, constants)?),
        ),
        lir::Instruction::BinOp { out, op, lhs, rhs } => {
            let value = match op {
                lir::BinOp::Eq | lir::BinOp::Ne => {
                    let equal = boolean_value(lhs, constants)? == boolean_value(rhs, constants)?;
                    if op == lir::BinOp::Eq { equal } else { !equal }
                }
                lir::BinOp::MachineEq(kind) => {
                    let lir::Value::MachineScalar(left) = scalar(lhs)? else {
                        return None;
                    };
                    let lir::Value::MachineScalar(right) = scalar(rhs)? else {
                        return None;
                    };
                    if left.kind() != kind || right.kind() != kind {
                        return None;
                    }
                    left == right
                }
            };
            (out, lir::Value::BoolConst(value))
        }
        _ => integers::result(instruction, scalar)?,
    };
    Some((LiveValue::Temp(out), KnownValue::Scalar(value)))
}

pub(super) fn known_value(
    value: lir::Value,
    constants: &HashMap<LiveValue, KnownValue>,
) -> Option<KnownValue> {
    match value {
        lir::Value::BoolConst(_) | lir::Value::IntegerConst(_) | lir::Value::MachineScalar(_) => {
            Some(KnownValue::Scalar(value))
        }
        lir::Value::Local(local) => constants.get(&LiveValue::Local(local)).copied(),
        lir::Value::Temp(temp) => constants.get(&LiveValue::Temp(temp)).copied(),
        _ => None,
    }
}

fn scalar_value(
    value: lir::Value,
    constants: &HashMap<LiveValue, KnownValue>,
) -> Option<lir::Value> {
    match known_value(value, constants)? {
        KnownValue::Scalar(value) => Some(value),
        KnownValue::Variant(_) => None,
    }
}

pub(super) fn boolean_value(
    value: lir::Value,
    constants: &HashMap<LiveValue, KnownValue>,
) -> Option<bool> {
    match scalar_value(value, constants)? {
        lir::Value::BoolConst(value) => Some(value),
        _ => None,
    }
}
