use scoop_ast as ast;
use scoop_hir as hir;

pub(in crate::globals) enum IntegerBinaryResult {
    Value(hir::ConstPropertyValue),
    DivisionByZero,
    Unsupported,
}

pub(crate) fn integer_wrapping_neg(value: hir::HirIntegerConstant) -> hir::HirIntegerConstant {
    integer_from_raw(
        value.kind(),
        0u64.wrapping_sub(value.raw_bits()) & value.kind().width().raw_mask(),
    )
}

pub(in crate::globals) fn convert_integer_constant(
    value: hir::HirIntegerConstant,
    target: hir::IntegerKind,
) -> hir::HirIntegerConstant {
    let source = value.kind();
    let extended = match source.signedness() {
        hir::IntegerSignedness::Signed => signed_value(source, value.raw_bits()) as u128,
        hir::IntegerSignedness::Unsigned => u128::from(value.raw_bits()),
    };
    integer_from_raw(
        target,
        (extended & u128::from(target.width().raw_mask())) as u64,
    )
}

pub(in crate::globals) fn evaluate_integer_no_gc_operation(
    operation: hir::NoGcIntegerOperation,
    left: hir::HirIntegerConstant,
    right: Option<hir::HirIntegerConstant>,
) -> Option<hir::ConstPropertyValue> {
    let kind = left.kind();
    let mask = kind.width().raw_mask();
    let left_raw = left.raw_bits();
    let value = match operation {
        hir::NoGcIntegerOperation::UnaryPlus => right.is_none().then_some(left_raw)?,
        hir::NoGcIntegerOperation::UnaryMinus => {
            right.is_none().then_some(0_u64.wrapping_sub(left_raw))?
        }
        hir::NoGcIntegerOperation::Inc => right.is_none().then_some(left_raw.wrapping_add(1))?,
        hir::NoGcIntegerOperation::Dec => right.is_none().then_some(left_raw.wrapping_sub(1))?,
        hir::NoGcIntegerOperation::Inv => right.is_none().then_some(!left_raw)?,
        hir::NoGcIntegerOperation::Add
        | hir::NoGcIntegerOperation::Sub
        | hir::NoGcIntegerOperation::Mul
        | hir::NoGcIntegerOperation::And
        | hir::NoGcIntegerOperation::Or
        | hir::NoGcIntegerOperation::Xor => {
            let right = right.filter(|right| right.kind() == kind)?.raw_bits();
            match operation {
                hir::NoGcIntegerOperation::Add => left_raw.wrapping_add(right),
                hir::NoGcIntegerOperation::Sub => left_raw.wrapping_sub(right),
                hir::NoGcIntegerOperation::Mul => left_raw.wrapping_mul(right),
                hir::NoGcIntegerOperation::And => left_raw & right,
                hir::NoGcIntegerOperation::Or => left_raw | right,
                hir::NoGcIntegerOperation::Xor => left_raw ^ right,
                _ => unreachable!("the outer match selected an integer binary operation"),
            }
        }
        hir::NoGcIntegerOperation::Shl
        | hir::NoGcIntegerOperation::Shr
        | hir::NoGcIntegerOperation::Ushr => {
            let right = right.filter(|right| right.kind() == hir::IntegerKind::SIGNED_64)?;
            let count = right.raw_bits() & u64::from(kind.width().bits() - 1);
            match operation {
                hir::NoGcIntegerOperation::Shl => left_raw.wrapping_shl(count as u32),
                hir::NoGcIntegerOperation::Shr
                    if kind.signedness() == hir::IntegerSignedness::Signed =>
                {
                    (signed_value(kind, left_raw) >> count) as u64
                }
                hir::NoGcIntegerOperation::Shr | hir::NoGcIntegerOperation::Ushr => {
                    left_raw >> count
                }
                _ => unreachable!("the outer match selected an integer shift operation"),
            }
        }
        hir::NoGcIntegerOperation::CompareTo => {
            let right = right.filter(|right| right.kind() == kind)?;
            let ordering = match kind.signedness() {
                hir::IntegerSignedness::Signed => {
                    signed_value(kind, left_raw).cmp(&signed_value(kind, right.raw_bits()))
                }
                hir::IntegerSignedness::Unsigned => left_raw.cmp(&right.raw_bits()),
            };
            let raw = match ordering {
                std::cmp::Ordering::Less => u64::MAX,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            };
            return Some(hir::ConstPropertyValue::Integer(
                hir::HirIntegerConstant::Signed64(raw),
            ));
        }
        hir::NoGcIntegerOperation::Equals => {
            let right = right.filter(|right| right.kind() == kind)?;
            return Some(hir::ConstPropertyValue::Boolean(
                left_raw == right.raw_bits(),
            ));
        }
    };
    Some(hir::ConstPropertyValue::Integer(integer_from_raw(
        kind,
        value & mask,
    )))
}

pub(crate) fn evaluate_hir_integer_constant(
    expression: &hir::Expr,
    setup: &[hir::Statement],
) -> Option<hir::HirIntegerConstant> {
    evaluate_hir_integer_constant_inner(expression, setup, &mut std::collections::HashSet::new())
}

fn evaluate_hir_integer_constant_inner(
    expression: &hir::Expr,
    setup: &[hir::Statement],
    visiting: &mut std::collections::HashSet<hir::LocalId>,
) -> Option<hir::HirIntegerConstant> {
    match &expression.kind {
        hir::ExprKind::IntegerLiteral(value) => Some(*value),
        hir::ExprKind::Local(local) => {
            if !visiting.insert(*local) {
                return None;
            }
            let initializer = setup.iter().rev().find_map(|statement| {
                let hir::StatementKind::ValDecl { pattern, init } = &statement.kind else {
                    return None;
                };
                matches!(pattern, hir::Pattern::Binding { local: binding } if binding == local)
                    .then_some(init)
            })?;
            let value = evaluate_hir_integer_constant_inner(initializer, setup, visiting);
            visiting.remove(local);
            value
        }
        hir::ExprKind::IntegerConversion {
            conversion,
            operand,
        } => {
            let value = evaluate_hir_integer_constant_inner(operand, setup, visiting)?;
            (value.kind() == conversion.source)
                .then(|| convert_integer_constant(value, conversion.target_kind))
        }
        hir::ExprKind::IntegerOperation {
            operation,
            arguments,
        } => {
            let (left, right) = match arguments {
                hir::HirIntegerOperationArguments::Unary(operand) => (
                    evaluate_hir_integer_constant_inner(operand, setup, visiting)?,
                    None,
                ),
                hir::HirIntegerOperationArguments::Binary { lhs, rhs } => (
                    evaluate_hir_integer_constant_inner(lhs, setup, visiting)?,
                    Some(evaluate_hir_integer_constant_inner(rhs, setup, visiting)?),
                ),
            };
            match operation {
                hir::IntegerOperation::NoGc {
                    kind, operation, ..
                } if left.kind() == *kind => {
                    match evaluate_integer_no_gc_operation(*operation, left, right)? {
                        hir::ConstPropertyValue::Integer(value) => Some(value),
                        hir::ConstPropertyValue::Boolean(_)
                        | hir::ConstPropertyValue::Char(_)
                        | hir::ConstPropertyValue::String(_) => None,
                    }
                }
                hir::IntegerOperation::Managed {
                    kind, operation, ..
                } if left.kind() == *kind && right.is_some_and(|right| right.kind() == *kind) => {
                    let operator = match operation {
                        hir::IntegerDivRem::Div => ast::BinOp::Div,
                        hir::IntegerDivRem::Rem => ast::BinOp::Rem,
                    };
                    match evaluate_integer_binary(operator, left, right?) {
                        IntegerBinaryResult::Value(hir::ConstPropertyValue::Integer(value)) => {
                            Some(value)
                        }
                        IntegerBinaryResult::Value(_)
                        | IntegerBinaryResult::DivisionByZero
                        | IntegerBinaryResult::Unsupported => None,
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

pub(in crate::globals) fn evaluate_integer_binary(
    operator: ast::BinOp,
    left: hir::HirIntegerConstant,
    right: hir::HirIntegerConstant,
) -> IntegerBinaryResult {
    if left.kind() != right.kind() {
        return IntegerBinaryResult::Unsupported;
    }
    let kind = left.kind();
    let left_raw = left.raw_bits();
    let right_raw = right.raw_bits();
    let mask = kind.width().raw_mask();
    let integer = |raw| {
        IntegerBinaryResult::Value(hir::ConstPropertyValue::Integer(integer_from_raw(
            kind,
            raw & mask,
        )))
    };
    match operator {
        ast::BinOp::Add => integer(left_raw.wrapping_add(right_raw)),
        ast::BinOp::Sub => integer(left_raw.wrapping_sub(right_raw)),
        ast::BinOp::Mul => integer(left_raw.wrapping_mul(right_raw)),
        ast::BinOp::Div | ast::BinOp::Rem if right_raw == 0 => IntegerBinaryResult::DivisionByZero,
        ast::BinOp::Div | ast::BinOp::Rem => {
            let raw = match kind.signedness() {
                hir::IntegerSignedness::Unsigned => {
                    if operator == ast::BinOp::Div {
                        left_raw / right_raw
                    } else {
                        left_raw % right_raw
                    }
                }
                hir::IntegerSignedness::Signed => {
                    let left = signed_value(kind, left_raw);
                    let right = signed_value(kind, right_raw);
                    let bits = kind.width().bits();
                    let minimum = -(1_i128 << (bits - 1));
                    let result = if left == minimum && right == -1 {
                        if operator == ast::BinOp::Div {
                            minimum
                        } else {
                            0
                        }
                    } else if operator == ast::BinOp::Div {
                        left / right
                    } else {
                        left % right
                    };
                    (result as u128 & u128::from(mask)) as u64
                }
            };
            integer(raw)
        }
        ast::BinOp::Eq | ast::BinOp::Ne => {
            let equal = left_raw == right_raw;
            IntegerBinaryResult::Value(hir::ConstPropertyValue::Boolean(
                if operator == ast::BinOp::Eq {
                    equal
                } else {
                    !equal
                },
            ))
        }
        ast::BinOp::Lt | ast::BinOp::Le | ast::BinOp::Gt | ast::BinOp::Ge => {
            let ordering = match kind.signedness() {
                hir::IntegerSignedness::Signed => {
                    signed_value(kind, left_raw).cmp(&signed_value(kind, right_raw))
                }
                hir::IntegerSignedness::Unsigned => left_raw.cmp(&right_raw),
            };
            let value = match operator {
                ast::BinOp::Lt => ordering.is_lt(),
                ast::BinOp::Le => ordering.is_le(),
                ast::BinOp::Gt => ordering.is_gt(),
                ast::BinOp::Ge => ordering.is_ge(),
                _ => unreachable!("the outer match selected an ordering operator"),
            };
            IntegerBinaryResult::Value(hir::ConstPropertyValue::Boolean(value))
        }
        _ => IntegerBinaryResult::Unsupported,
    }
}

pub(super) fn signed_value(kind: hir::IntegerKind, raw: u64) -> i128 {
    let bits = kind.width().bits();
    let sign = 1_u64 << (bits - 1);
    if raw & sign == 0 {
        i128::from(raw)
    } else {
        i128::from(raw) - (1_i128 << bits)
    }
}

fn integer_from_raw(kind: hir::IntegerKind, raw: u64) -> hir::HirIntegerConstant {
    match kind {
        hir::IntegerKind::SIGNED_8 => hir::HirIntegerConstant::Signed8(raw as u8),
        hir::IntegerKind::SIGNED_16 => hir::HirIntegerConstant::Signed16(raw as u16),
        hir::IntegerKind::SIGNED_32 => hir::HirIntegerConstant::Signed32(raw as u32),
        hir::IntegerKind::SIGNED_64 => hir::HirIntegerConstant::Signed64(raw),
        hir::IntegerKind::UNSIGNED_8 => hir::HirIntegerConstant::Unsigned8(raw as u8),
        hir::IntegerKind::UNSIGNED_16 => hir::HirIntegerConstant::Unsigned16(raw as u16),
        hir::IntegerKind::UNSIGNED_32 => hir::HirIntegerConstant::Unsigned32(raw as u32),
        hir::IntegerKind::UNSIGNED_64 => hir::HirIntegerConstant::Unsigned64(raw),
    }
}
