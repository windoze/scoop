use super::*;
use scoop_ast::Expr;

impl Parser {
    pub(super) fn parse_return(&mut self) -> Result<Statement, Diagnostic> {
        let Expr::Return { value, span } = self.parse_return_expression()? else {
            unreachable!("return parser constructs a return expression")
        };
        Ok(Statement {
            kind: StatementKind::Return {
                value: value.map(|value| *value),
            },
            span,
        })
    }

    pub(crate) fn parse_return_expression(&mut self) -> Result<Expr, Diagnostic> {
        let keyword = self.bump();
        self.reject_jump_label(&keyword, "return")?;
        let token = self.peek();
        let has_value = !token.newline_before
            && !matches!(
                token.kind,
                TokenKind::RBrace
                    | TokenKind::RParen
                    | TokenKind::RBracket
                    | TokenKind::Comma
                    | TokenKind::Semicolon
                    | TokenKind::Eof
            );
        let value = if has_value {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        let end = value
            .as_ref()
            .map_or(keyword.span.end, |value| value.span().end);
        Ok(Expr::Return {
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
        let value = self.parse_expr()?;
        let span = Span::new(keyword.span.start, value.span().end);
        Ok(Expr::Throw {
            value: Box::new(value),
            span,
        })
    }
}
