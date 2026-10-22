use super::*;

impl Parser {
    /// Only unfinished operands need an additional path from the real statement.
    pub(crate) fn parse_statement_operand(
        &mut self,
        statement_start: u32,
        context: impl FnOnce() -> ast::RecoveredOperandContext,
    ) -> Result<ast::Expr, ast::Diagnostic> {
        let start = self.peek().span.start;
        let checkpoint = self.checkpoint();
        let result = self.parse_expr();
        if result.is_err() || !self.fragment_is_complete(checkpoint) {
            self.syntax
                .events
                .push(Event::Body(BodyEvent::Operand(ast::RecoveredOperand {
                    block: None,
                    statement_start,
                    expression_start: start,
                    context: context(),
                })));
        }
        result
    }
}
