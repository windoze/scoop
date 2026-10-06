use scoop_ast as ast;
use scoop_hir as hir;

use super::consts::{
    ConstIntegerIntrinsicKind, IntegerBinaryResult, convert_integer_constant,
    evaluate_integer_binary, evaluate_integer_no_gc_operation,
};
use crate::Lowerer;

mod floating;
mod integer_calls;
use super::consts::{evaluate_float_binary, evaluate_float_unary, float_binary_operator};

struct StaticValue {
    value: hir::ConstPropertyValue,
    ty: hir::TypeId,
}

impl Lowerer {
    pub(super) fn static_property_constant(
        &mut self,
        expression: &ast::Expr,
        expected: hir::TypeId,
    ) -> Option<hir::HirConstantImage> {
        let mut probe = self.clone();
        let value = probe.evaluate_static_property_constant(expression, expected);
        if value.is_some() {
            self.dependencies = probe.dependencies;
        }
        value
    }

    fn evaluate_static_property_constant(
        &mut self,
        expression: &ast::Expr,
        expected: hir::TypeId,
    ) -> Option<hir::HirConstantImage> {
        if let Some(variant) = self.static_unit_variant_constant(expression, expected) {
            return Some(variant);
        }
        let evaluated = self.evaluate_static_value(expression, Some(expected))?;
        if !self.types_equal(evaluated.ty, expected) {
            return None;
        }
        Some(match evaluated.value {
            hir::ConstPropertyValue::Integer(value) => hir::HirConstantImage::Integer(value),
            hir::ConstPropertyValue::Float(value) => hir::HirConstantImage::Float(value),
            hir::ConstPropertyValue::Char(value) => hir::HirConstantImage::Char(value),
            hir::ConstPropertyValue::Boolean(value) => hir::HirConstantImage::Boolean(value),
            hir::ConstPropertyValue::String(value) => hir::HirConstantImage::String(value),
        })
    }

    fn evaluate_static_value(
        &mut self,
        expression: &ast::Expr,
        expected: Option<hir::TypeId>,
    ) -> Option<StaticValue> {
        match expression {
            ast::Expr::IntLiteral(literal) => {
                let expression =
                    self.lower_integer_literal(*literal, expected, false, literal.span)?;
                let hir::ExprKind::IntegerLiteral(value) = expression.kind else {
                    unreachable!("literal lowering produces an integer constant")
                };
                Some(StaticValue {
                    value: hir::ConstPropertyValue::Integer(value),
                    ty: expression.ty,
                })
            }
            ast::Expr::FloatLiteral(literal) => {
                let expression =
                    self.lower_float_literal(literal, expected, false, literal.span)?;
                let hir::ExprKind::FloatLiteral(value) = expression.kind else {
                    unreachable!("float literal lowering produces a floating constant")
                };
                Some(StaticValue {
                    value: hir::ConstPropertyValue::Float(value),
                    ty: expression.ty,
                })
            }
            ast::Expr::CharLiteral { value, .. } => Some(StaticValue {
                value: hir::ConstPropertyValue::Char(*value),
                ty: self.core_character_type().ok()?,
            }),
            ast::Expr::BoolLiteral { value, .. } => Some(StaticValue {
                value: hir::ConstPropertyValue::Boolean(*value),
                ty: self.boolean,
            }),
            ast::Expr::StringLiteral { value, .. } => Some(StaticValue {
                value: hir::ConstPropertyValue::String(value.clone()),
                ty: self.string,
            }),
            ast::Expr::FieldAccess(access) => {
                let (value, ty) = self
                    .lower_imported_const_reference(access, expected)
                    .ok()??;
                Some(StaticValue { value, ty })
            }
            ast::Expr::Var(name) => {
                let crate::imports::lookup::LookupResult::Unique(origin) =
                    self.lookup_value_origin(&name.text)
                else {
                    return None;
                };
                if let crate::imports::lookup::values::ValueOrigin::Dependency(binding) = origin {
                    let imported = self.select_imported_dependency_constant(&binding, name.span)?;
                    return Some(StaticValue {
                        value: imported.value,
                        ty: imported.ty,
                    });
                }
                let crate::imports::lookup::values::ValueTarget::Property(property) =
                    self.materialized_value_target(&origin)?
                else {
                    return None;
                };
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
                if *op == ast::UnOp::Neg
                    && let ast::Expr::IntLiteral(literal) = &**operand
                    && matches!(
                        literal.suffix,
                        ast::IntegerSuffix::None | ast::IntegerSuffix::Long
                    )
                {
                    let expression =
                        self.lower_integer_literal(*literal, expected, true, expression.span())?;
                    let hir::ExprKind::IntegerLiteral(value) = expression.kind else {
                        unreachable!("literal lowering produces an integer constant")
                    };
                    return Some(StaticValue {
                        value: hir::ConstPropertyValue::Integer(value),
                        ty: expression.ty,
                    });
                }
                let operand = self.evaluate_static_value(operand, expected)?;
                match (*op, operand.value) {
                    (ast::UnOp::Plus | ast::UnOp::Neg, hir::ConstPropertyValue::Float(value)) => {
                        Some(StaticValue {
                            value: evaluate_float_unary(
                                if *op == ast::UnOp::Plus {
                                    hir::FloatUnaryOperator::Plus
                                } else {
                                    hir::FloatUnaryOperator::Negate
                                },
                                value,
                            ),
                            ty: operand.ty,
                        })
                    }
                    (ast::UnOp::Plus | ast::UnOp::Neg, hir::ConstPropertyValue::Integer(value)) => {
                        let hir::Type::Integer(kind) = self.types[operand.ty] else {
                            return None;
                        };
                        let operation = match op {
                            ast::UnOp::Plus => hir::NoGcIntegerOperation::UnaryPlus,
                            ast::UnOp::Neg => hir::NoGcIntegerOperation::UnaryMinus,
                            ast::UnOp::Not => {
                                unreachable!("the outer match selected an integer unary operator")
                            }
                        };
                        if !self.const_integer_operation_available(
                            hir::IntegerIntrinsicKind::NoGcOperation { kind, operation },
                        ) {
                            return None;
                        }
                        Some(StaticValue {
                            value: evaluate_integer_no_gc_operation(operation, value, None)?,
                            ty: self.integer_no_gc_result_type(kind, operation),
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
                let equality = matches!(op, ast::BinOp::Eq | ast::BinOp::Ne);
                let operand_kind = if equality {
                    self.select_static_equality_integer_kind(lhs, rhs)
                } else {
                    self.select_const_binary_literal_kind(*op, lhs, expected, |kind| {
                        self.probe_static_integer_kind(rhs, Some(kind)) == Some(kind)
                    })
                };
                let operand_expected = operand_kind
                    .map(|kind| self.integer_type(kind))
                    .or_else(|| expected.filter(|ty| self.float_kind(*ty).is_some()));
                let lhs = self.evaluate_static_value(lhs, operand_expected)?;
                let rhs_expected = Some(lhs.ty);
                let rhs = self.evaluate_static_value(rhs, rhs_expected)?;
                if !self.types_equal(lhs.ty, rhs.ty) {
                    return None;
                }
                let value = match (lhs.value, rhs.value) {
                    (
                        hir::ConstPropertyValue::Float(left),
                        hir::ConstPropertyValue::Float(right),
                    ) => float_binary_operator(*op)
                        .map(|operation| evaluate_float_binary(operation, left, right)),
                    (
                        hir::ConstPropertyValue::Integer(left),
                        hir::ConstPropertyValue::Integer(right),
                    ) => match self.evaluate_typed_integer_binary_operator(*op, left, right) {
                        IntegerBinaryResult::Value(value) => Some(value),
                        IntegerBinaryResult::DivisionByZero | IntegerBinaryResult::Unsupported => {
                            None
                        }
                    },
                    (hir::ConstPropertyValue::Char(left), hir::ConstPropertyValue::Char(right)) => {
                        super::consts::evaluate_character_binary(*op, left, right)
                    }
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
                    hir::ConstPropertyValue::Integer(_)
                    | hir::ConstPropertyValue::Float(_)
                    | hir::ConstPropertyValue::Char(_) => lhs.ty,
                };
                Some(StaticValue { value, ty })
            }
            ast::Expr::InfixCall {
                lhs, target, rhs, ..
            } => self.evaluate_static_integer_infix(lhs, target, rhs, expected),
            ast::Expr::MethodCall {
                receiver,
                name,
                navigation,
                type_args,
                args,
                ..
            } => self.evaluate_static_integer_method(
                receiver,
                name,
                *navigation,
                type_args,
                args,
                expected,
            ),
            _ => None,
        }
    }
}
