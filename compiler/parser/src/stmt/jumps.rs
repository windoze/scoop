use super::*;
use scoop_ast::Expr;

impl Parser {
    pub(super) fn parse_loop_jump(&mut self) -> Result<Statement, Diagnostic> {
        let Expr::LoopJump { kind, label, span } = self.parse_loop_jump_expression()? else {
            unreachable!("loop jump parser constructs a loop jump expression")
        };
        let kind = match kind {
            scoop_ast::LoopJumpKind::Break => StatementKind::Break { label },
            scoop_ast::LoopJumpKind::Continue => StatementKind::Continue { label },
        };
        Ok(Statement { kind, span })
    }

    pub(crate) fn parse_loop_jump_expression(&mut self) -> Result<Expr, Diagnostic> {
        let keyword = self.bump();
        let kind = match keyword.kind {
            TokenKind::Break => scoop_ast::LoopJumpKind::Break,
            TokenKind::Continue => scoop_ast::LoopJumpKind::Continue,
            _ => unreachable!("a loop jump starts with break or continue"),
        };
        let label = if !self.peek().newline_before && matches!(self.peek().kind, TokenKind::At) {
            self.bump();
            if self.peek().newline_before {
                return self.unexpected("loop label on the same line");
            }
            Some(self.expect_ident("loop label")?)
        } else {
            None
        };
        let end = label
            .as_ref()
            .map_or(keyword.span.end, |label| label.span.end);
        Ok(Expr::LoopJump {
            kind,
            label,
            span: Span::new(keyword.span.start, end),
        })
    }

    pub(super) fn parse_return(&mut self) -> Result<Statement, Diagnostic> {
        let Expr::Return { label, value, span } = self.parse_return_expression()? else {
            unreachable!("return parser constructs a return expression")
        };
        Ok(Statement {
            kind: StatementKind::Return {
                label,
                value: value.map(|value| *value),
            },
            span,
        })
    }

    pub(crate) fn parse_return_expression(&mut self) -> Result<Expr, Diagnostic> {
        let keyword = self.bump();
        let label = if !self.peek().newline_before && matches!(self.peek().kind, TokenKind::At) {
            self.bump();
            if self.peek().newline_before {
                return self.unexpected("return label on the same line");
            }
            Some(self.expect_ident("return label")?)
        } else {
            None
        };
        let token = self.peek();
        let has_value = !token.newline_before
            && !matches!(
                token.kind,
                TokenKind::RBrace
                    | TokenKind::RParen
                    | TokenKind::RBracket
                    | TokenKind::Comma
                    | TokenKind::Semicolon
                    | TokenKind::Else
                    | TokenKind::Eof
            );
        let value = if has_value {
            Some(Box::new(self.parse_statement_operand(
                keyword.span.start,
                || scoop_ast::RecoveredOperandContext::Return {
                    label: label.clone(),
                },
            )?))
        } else {
            None
        };
        let end = value.as_ref().map_or_else(
            || {
                label
                    .as_ref()
                    .map_or(keyword.span.end, |label| label.span.end)
            },
            |value| value.span().end,
        );
        Ok(Expr::Return {
            label,
            value,
            span: Span::new(keyword.span.start, end),
        })
    }

    pub(super) fn parse_throw(&mut self) -> Result<Statement, Diagnostic> {
        let Expr::Throw { value, span } = self.parse_throw_expression()? else {
            unreachable!("throw parser constructs a throw expression")
        };
        Ok(Statement {
            kind: StatementKind::Throw(*value),
            span,
        })
    }

    pub(crate) fn parse_throw_expression(&mut self) -> Result<Expr, Diagnostic> {
        let keyword = self.bump();
        let value = self.parse_statement_operand(keyword.span.start, || {
            scoop_ast::RecoveredOperandContext::Throw
        })?;
        let span = Span::new(keyword.span.start, value.span().end);
        Ok(Expr::Throw {
            value: Box::new(value),
            span,
        })
    }
}
