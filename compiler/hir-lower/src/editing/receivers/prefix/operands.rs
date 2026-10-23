use super::*;

impl<F: Fn() -> bool> PrefixQuery<'_, F> {
    pub(super) fn operand(
        &mut self,
        lowerer: &mut Lowerer,
        operand: &ast::RecoveredOperand,
        available: bool,
    ) -> Result<(), EditingCancelled> {
        if !available {
            return self.expression(lowerer, operand.block, operand.expression_start, false);
        }
        let (available, expected) = match &operand.context {
            ast::RecoveredOperandContext::Initializer {
                annotation: Some(ty),
            } => {
                let expected = lowerer.resolve_type_ref(ty);
                (expected.is_some(), expected)
            }
            ast::RecoveredOperandContext::Return { label } => {
                let available = !self.expression_body_return
                    && lowerer
                        .check_return_context(
                            label.as_ref(),
                            ast::Span::new(operand.statement_start, operand.expression_start),
                        )
                        .is_some();
                (
                    available,
                    lowerer
                        .return_inference
                        .is_none()
                        .then_some(lowerer.current_return_ty),
                )
            }
            ast::RecoveredOperandContext::Initializer { annotation: None }
            | ast::RecoveredOperandContext::Throw
            | ast::RecoveredOperandContext::IfCondition
            | ast::RecoveredOperandContext::ForIterable
            | ast::RecoveredOperandContext::WhenSubject => (true, None),
            ast::RecoveredOperandContext::WhileCondition { label } => {
                return self.while_condition(lowerer, operand, label.as_ref());
            }
        };
        self.expression_with_expected(
            lowerer,
            operand.block,
            operand.expression_start,
            available,
            expected,
        )
    }
}
