use super::*;

impl CfgLowerer<'_> {
    pub(super) fn lower_variant_condition(
        &mut self,
        expr: &smir::Expr,
        span: Span,
    ) -> Option<mir::Expr> {
        if let smir::ExprKind::VariantTest { operand, variant } = &expr.kind
            && !matches!(operand.kind, smir::ExprKind::Local(_))
            && same_value_projection(operand, operand, &self.locals)
        {
            // Capture at the test, inside any enclosing payload guard. The
            // short-circuit caller bounds reuse to this evaluation scope.
            let value = self.lower_variant_operand(operand, span)?;
            let value = self.save_operand(value, span);
            self.variant_operands
                .push(((**operand).clone(), value.clone()));
            return Some(
                mir::Expr::variant_test(self.enums, value, *variant)
                    .expect("a captured enum retains its checked variant type"),
            );
        }
        self.lower_expr(expr, span)
    }

    pub(super) fn lower_variant_operand(
        &mut self,
        expr: &smir::Expr,
        span: Span,
    ) -> Option<mir::Expr> {
        if let Some((_, value)) = self
            .variant_operands
            .iter()
            .rev()
            .find(|(source, _)| same_value_projection(source, expr, &self.locals))
        {
            return Some(value.clone());
        }
        self.lower_expr(expr, span)
    }
}

fn same_value_projection(
    left: &smir::Expr,
    right: &smir::Expr,
    locals: &Arena<mir::Local>,
) -> bool {
    if left.ty != right.ty {
        return false;
    }
    match (&left.kind, &right.kind) {
        (smir::ExprKind::Local(left), smir::ExprKind::Local(right)) => {
            left == right && !locals[*left].mutable
        }
        (
            smir::ExprKind::FieldAccess {
                receiver: left,
                index: left_index,
            },
            smir::ExprKind::FieldAccess {
                receiver: right,
                index: right_index,
            },
        ) => {
            left_index == right_index
                && matches!(left.ty, mir::Type::Tuple(_) | mir::Type::Struct(_))
                && same_value_projection(left, right, locals)
        }
        (
            smir::ExprKind::VariantPayloadProject {
                operand: left,
                field: left_field,
            },
            smir::ExprKind::VariantPayloadProject {
                operand: right,
                field: right_field,
            },
        ) => left_field == right_field && same_value_projection(left, right, locals),
        _ => false,
    }
}
