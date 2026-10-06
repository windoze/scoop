use super::*;

impl Lowerer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn evaluate_const_binary(
        &mut self,
        operator: ast::BinOp,
        lhs: &ast::Expr,
        rhs: &ast::Expr,
        expected: Option<hir::TypeId>,
        file: usize,
        declarations: &[PendingConst<'_>],
        ordinary: &[PendingOrdinary<'_>],
        states: &mut [ConstState],
        stack: &mut Vec<usize>,
        span: ast::Span,
    ) -> Option<EvaluatedConst> {
        if matches!(operator, ast::BinOp::And | ast::BinOp::Or) {
            let lhs = self.evaluate_const_expression(
                lhs,
                Some(self.boolean),
                file,
                declarations,
                ordinary,
                states,
                stack,
            );
            let rhs = self.evaluate_const_expression(
                rhs,
                Some(self.boolean),
                file,
                declarations,
                ordinary,
                states,
                stack,
            );
            let (Some(lhs), Some(rhs)) = (lhs, rhs) else {
                return None;
            };
            if !self.types_equal(lhs.ty, rhs.ty) {
                self.error(
                    span,
                    format!(
                        "const operator operands must have the same type, found {} and {}",
                        self.type_name(lhs.ty),
                        self.type_name(rhs.ty)
                    ),
                );
                return None;
            }
            let (hir::ConstPropertyValue::Boolean(lhs), hir::ConstPropertyValue::Boolean(rhs)) =
                (lhs.value, rhs.value)
            else {
                self.error(
                    span,
                    "boolean const operator requires Boolean operands".to_string(),
                );
                return None;
            };
            let value = match operator {
                ast::BinOp::And => lhs && rhs,
                ast::BinOp::Or => lhs || rhs,
                _ => unreachable!("the outer match selected a boolean short-circuit operator"),
            };
            return Some(EvaluatedConst {
                value: hir::ConstPropertyValue::Boolean(value),
                ty: self.boolean,
            });
        }

        let equality = matches!(operator, ast::BinOp::Eq | ast::BinOp::Ne);
        let operand_kind = if equality {
            self.select_const_equality_integer_kind(
                lhs,
                rhs,
                file,
                declarations,
                ordinary,
                states,
                stack,
            )
        } else {
            self.select_const_binary_literal_kind(operator, lhs, expected, |kind| {
                self.probe_const_integer_kind(
                    rhs,
                    Some(kind),
                    file,
                    declarations,
                    ordinary,
                    states,
                    stack,
                ) == Some(kind)
            })
        };
        let float_kind = self.select_const_float_binary_kind(operator, lhs, expected, |kind| {
            self.probe_const_float_kind(rhs, kind, file, declarations, ordinary, states, stack)
                == Some(kind)
        });
        let operand_expected = operand_kind
            .map(|kind| self.integer_type(kind))
            .or_else(|| float_kind.and_then(|kind| self.core_float_type(kind).ok()))
            .or_else(|| expected.filter(|ty| self.float_kind(*ty).is_some()));
        let lhs = self.evaluate_const_expression(
            lhs,
            operand_expected,
            file,
            declarations,
            ordinary,
            states,
            stack,
        )?;
        let rhs_expected = Some(lhs.ty);
        let rhs = self.evaluate_const_expression(
            rhs,
            rhs_expected,
            file,
            declarations,
            ordinary,
            states,
            stack,
        )?;
        if !self.types_equal(lhs.ty, rhs.ty) {
            self.error(
                span,
                format!(
                    "const operator operands must have the same type, found {} and {}",
                    self.type_name(lhs.ty),
                    self.type_name(rhs.ty)
                ),
            );
            return None;
        }

        let result = match (lhs.value, rhs.value) {
            (hir::ConstPropertyValue::Float(left), hir::ConstPropertyValue::Float(right)) => {
                float_binary_operator(operator)
                    .map(|operation| evaluate_float_binary(operation, left, right))
            }
            (hir::ConstPropertyValue::Integer(left), hir::ConstPropertyValue::Integer(right)) => {
                let hir::Type::Integer(kind) = self.types[lhs.ty] else {
                    unreachable!("integer constant values have integer types")
                };
                debug_assert_eq!(left.kind(), kind);
                debug_assert_eq!(right.kind(), kind);
                match self.evaluate_typed_integer_binary_operator(operator, left, right) {
                    IntegerBinaryResult::Value(value) => Some(value),
                    IntegerBinaryResult::DivisionByZero => {
                        self.error(span, "division by zero in const initializer".to_string());
                        return None;
                    }
                    IntegerBinaryResult::Unsupported => None,
                }
            }
            (hir::ConstPropertyValue::Char(left), hir::ConstPropertyValue::Char(right)) => {
                evaluate_character_binary(operator, left, right)
            }
            (hir::ConstPropertyValue::Boolean(left), hir::ConstPropertyValue::Boolean(right)) => {
                match operator {
                    ast::BinOp::Eq => Some(hir::ConstPropertyValue::Boolean(left == right)),
                    ast::BinOp::Ne => Some(hir::ConstPropertyValue::Boolean(left != right)),
                    _ => None,
                }
            }
            (hir::ConstPropertyValue::String(left), hir::ConstPropertyValue::String(right)) => {
                match operator {
                    ast::BinOp::Add => Some(hir::ConstPropertyValue::String(left + &right)),
                    ast::BinOp::Eq => Some(hir::ConstPropertyValue::Boolean(left == right)),
                    ast::BinOp::Ne => Some(hir::ConstPropertyValue::Boolean(left != right)),
                    ast::BinOp::Lt => Some(hir::ConstPropertyValue::Boolean(left < right)),
                    ast::BinOp::Le => Some(hir::ConstPropertyValue::Boolean(left <= right)),
                    ast::BinOp::Gt => Some(hir::ConstPropertyValue::Boolean(left > right)),
                    ast::BinOp::Ge => Some(hir::ConstPropertyValue::Boolean(left >= right)),
                    _ => None,
                }
            }
            _ => None,
        };
        let Some(value) = result else {
            self.error(
                span,
                "invalid binary operator in const initializer".to_string(),
            );
            return None;
        };
        let ty = match value {
            hir::ConstPropertyValue::Boolean(_) => self.boolean,
            hir::ConstPropertyValue::String(_) => self.string,
            hir::ConstPropertyValue::Integer(_)
            | hir::ConstPropertyValue::Float(_)
            | hir::ConstPropertyValue::Char(_) => lhs.ty,
        };
        Some(EvaluatedConst { value, ty })
    }
}
