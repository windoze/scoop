use super::*;

impl Parser {
    pub(super) fn parse_when_subject(&mut self, start: u32) -> Result<WhenSubject, Diagnostic> {
        if matches!(self.peek().kind, TokenKind::LBrace) {
            return Ok(WhenSubject::Absent);
        }
        self.expect("`(`", |kind| matches!(kind, TokenKind::LParen))?;
        let subject = match self.peek().kind {
            TokenKind::Var => {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "a when subject declaration must use `val`, not `var`",
                ));
            }
            TokenKind::Val => {
                let checkpoint = self.checkpoint();
                let declaration_start = self.peek().span.start;
                let result = self.parse_val_decl();
                if result.is_err() || !self.fragment_is_complete(checkpoint) {
                    self.record_recovered_operand(ast::RecoveredOperand {
                        block: None,
                        statement_start: start,
                        expression_start: declaration_start,
                        context: ast::RecoveredOperandContext::WhenSubject,
                    });
                }
                let declaration = result?;
                let StatementKind::ValDecl(value) = declaration.kind else {
                    return Err(Diagnostic::at(
                        declaration.span,
                        "a when subject declaration cannot be delegated",
                    ));
                };
                WhenSubject::Declaration(Box::new(value))
            }
            _ => WhenSubject::Expression(
                self.parse_statement_operand(start, || ast::RecoveredOperandContext::WhenSubject)?,
            ),
        };
        self.expect("`)`", |kind| matches!(kind, TokenKind::RParen))?;
        Ok(subject)
    }
}
