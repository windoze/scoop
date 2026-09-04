use scoop_ast as ast;
use scoop_hir as hir;

use crate::Lowerer;

struct StaticValue {
    value: hir::ConstPropertyValue,
    ty: hir::TypeId,
}

impl Lowerer {
    pub(super) fn static_property_constant(
        &mut self,
        expression: &ast::Expr,
        expected: hir::TypeId,
    ) -> Option<hir::ConstantValue> {
        if let ast::Expr::Var(name) = expression
            && name.text == "None"
        {
            return self.static_none_constant(expected);
        }
        let evaluated = self.evaluate_static_value(expression, Some(expected))?;
        if !self.types_equal(evaluated.ty, expected) {
            return None;
        }
        Some(match evaluated.value {
            hir::ConstPropertyValue::Integer(value) => hir::ConstantValue::Int(value),
            hir::ConstPropertyValue::Boolean(value) => hir::ConstantValue::Bool(value),
            hir::ConstPropertyValue::String(value) => hir::ConstantValue::String(value),
        })
    }

    fn evaluate_static_value(
        &mut self,
        expression: &ast::Expr,
        expected: Option<hir::TypeId>,
    ) -> Option<StaticValue> {
        match expression {
            ast::Expr::IntLiteral { value, .. } => Some(StaticValue {
                value: hir::ConstPropertyValue::Integer(*value),
                ty: expected
                    .filter(|ty| matches!(self.types[*ty], hir::Type::Int | hir::Type::UInt))
                    .unwrap_or(self.int),
            }),
            ast::Expr::BoolLiteral { value, .. } => Some(StaticValue {
                value: hir::ConstPropertyValue::Boolean(*value),
                ty: self.boolean,
            }),
            ast::Expr::StringLiteral { value, .. } => Some(StaticValue {
                value: hir::ConstPropertyValue::String(value.clone()),
                ty: self.string,
            }),
            ast::Expr::Var(name) => {
                let property = self.visible_property(&name.text, None)?;
                let declaration = self.properties[property].clone();
                let hir::PropertyRepresentation::Const { value } = declaration.representation
                else {
                    return None;
                };
                Some(StaticValue {
                    value,
                    ty: declaration.ty,
                })
            }
            ast::Expr::Unary { op, operand, .. } => {
                let operand = self.evaluate_static_value(operand, expected)?;
                match (*op, operand.value) {
                    (ast::UnOp::Plus, hir::ConstPropertyValue::Integer(value)) => {
                        Some(StaticValue {
                            value: hir::ConstPropertyValue::Integer(value),
                            ty: operand.ty,
                        })
                    }
                    (ast::UnOp::Neg, hir::ConstPropertyValue::Integer(value))
                        if matches!(self.types[operand.ty], hir::Type::Int) =>
                    {
                        Some(StaticValue {
                            value: hir::ConstPropertyValue::Integer(value.wrapping_neg()),
                            ty: operand.ty,
                        })
                    }
                    (ast::UnOp::Not, hir::ConstPropertyValue::Boolean(value)) => {
                        Some(StaticValue {
                            value: hir::ConstPropertyValue::Boolean(!value),
                            ty: self.boolean,
                        })
                    }
                    _ => None,
                }
            }
            ast::Expr::Binary { op, lhs, rhs, .. } => {
                let operand_expected = match op {
                    ast::BinOp::Add
                    | ast::BinOp::Sub
                    | ast::BinOp::Mul
                    | ast::BinOp::Div
                    | ast::BinOp::Rem => expected,
                    _ => None,
                };
                let lhs = self.evaluate_static_value(lhs, operand_expected)?;
                let rhs = self.evaluate_static_value(rhs, Some(lhs.ty))?;
                if !self.types_equal(lhs.ty, rhs.ty) {
                    return None;
                }
                let value = match (lhs.value, rhs.value) {
                    (
                        hir::ConstPropertyValue::Integer(left),
                        hir::ConstPropertyValue::Integer(right),
                    ) => integer_binary(
                        *op,
                        left,
                        right,
                        matches!(self.types[lhs.ty], hir::Type::UInt),
                    ),
                    (
                        hir::ConstPropertyValue::Boolean(left),
                        hir::ConstPropertyValue::Boolean(right),
                    ) => match op {
                        ast::BinOp::And => Some(hir::ConstPropertyValue::Boolean(left && right)),
                        ast::BinOp::Or => Some(hir::ConstPropertyValue::Boolean(left || right)),
                        ast::BinOp::Eq => Some(hir::ConstPropertyValue::Boolean(left == right)),
                        ast::BinOp::Ne => Some(hir::ConstPropertyValue::Boolean(left != right)),
                        _ => None,
                    },
                    (
                        hir::ConstPropertyValue::String(left),
                        hir::ConstPropertyValue::String(right),
                    ) => match op {
                        ast::BinOp::Add => Some(hir::ConstPropertyValue::String(left + &right)),
                        ast::BinOp::Eq => Some(hir::ConstPropertyValue::Boolean(left == right)),
                        ast::BinOp::Ne => Some(hir::ConstPropertyValue::Boolean(left != right)),
                        ast::BinOp::Lt => Some(hir::ConstPropertyValue::Boolean(left < right)),
                        ast::BinOp::Le => Some(hir::ConstPropertyValue::Boolean(left <= right)),
                        ast::BinOp::Gt => Some(hir::ConstPropertyValue::Boolean(left > right)),
                        ast::BinOp::Ge => Some(hir::ConstPropertyValue::Boolean(left >= right)),
                        _ => None,
                    },
                    _ => None,
                }?;
                let ty = match value {
                    hir::ConstPropertyValue::Boolean(_) => self.boolean,
                    hir::ConstPropertyValue::String(_) => self.string,
                    hir::ConstPropertyValue::Integer(_) => lhs.ty,
                };
                Some(StaticValue { value, ty })
            }
            _ => None,
        }
    }
}

fn integer_binary(
    operator: ast::BinOp,
    left: i64,
    right: i64,
    unsigned: bool,
) -> Option<hir::ConstPropertyValue> {
    let value = match operator {
        ast::BinOp::Add => hir::ConstPropertyValue::Integer(left.wrapping_add(right)),
        ast::BinOp::Sub => hir::ConstPropertyValue::Integer(left.wrapping_sub(right)),
        ast::BinOp::Mul => hir::ConstPropertyValue::Integer(left.wrapping_mul(right)),
        ast::BinOp::Div if right != 0 && unsigned => {
            hir::ConstPropertyValue::Integer(((left as u64) / (right as u64)) as i64)
        }
        ast::BinOp::Rem if right != 0 && unsigned => {
            hir::ConstPropertyValue::Integer(((left as u64) % (right as u64)) as i64)
        }
        ast::BinOp::Div if right != 0 => {
            hir::ConstPropertyValue::Integer(left.checked_div(right).unwrap_or(i64::MIN))
        }
        ast::BinOp::Rem if right != 0 => {
            hir::ConstPropertyValue::Integer(left.checked_rem(right).unwrap_or(0))
        }
        ast::BinOp::Div | ast::BinOp::Rem => return None,
        ast::BinOp::Eq => hir::ConstPropertyValue::Boolean(left == right),
        ast::BinOp::Ne => hir::ConstPropertyValue::Boolean(left != right),
        ast::BinOp::Lt if unsigned => {
            hir::ConstPropertyValue::Boolean((left as u64) < (right as u64))
        }
        ast::BinOp::Le if unsigned => {
            hir::ConstPropertyValue::Boolean((left as u64) <= (right as u64))
        }
        ast::BinOp::Gt if unsigned => {
            hir::ConstPropertyValue::Boolean((left as u64) > (right as u64))
        }
        ast::BinOp::Ge if unsigned => {
            hir::ConstPropertyValue::Boolean((left as u64) >= (right as u64))
        }
        ast::BinOp::Lt => hir::ConstPropertyValue::Boolean(left < right),
        ast::BinOp::Le => hir::ConstPropertyValue::Boolean(left <= right),
        ast::BinOp::Gt => hir::ConstPropertyValue::Boolean(left > right),
        ast::BinOp::Ge => hir::ConstPropertyValue::Boolean(left >= right),
        _ => return None,
    };
    Some(value)
}
