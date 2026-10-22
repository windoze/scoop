use super::*;

impl<F: Fn() -> bool> PrefixQuery<'_, F> {
    pub(super) fn callable_literal(
        &mut self,
        lowerer: &mut Lowerer,
        literal: &ast::RecoveredCallableLiteral,
        available: bool,
        expected: Option<crate::expr::ExpressionExpectation>,
    ) -> Result<(), EditingCancelled> {
        check_cancelled(self.is_cancelled)?;
        if !available {
            return self.block(lowerer, literal.body, false);
        }
        let outer_expression_body = self.expression_body_return;
        self.capture_definitions
            .extend(lowerer.locals.iter().filter_map(|(_, local)| {
                lowerer
                    .editing_local_reference(local)
                    .map(|reference| (reference.binding, reference))
            }));
        let result = lowerer.with_recovered_callable_context(literal, expected, |state| {
            self.expression_body_return = false;
            self.block(state, literal.body, true)
        });
        self.expression_body_return = outer_expression_body;
        match result {
            Some(result) => result,
            None => self.block(lowerer, literal.body, false),
        }
    }
}
