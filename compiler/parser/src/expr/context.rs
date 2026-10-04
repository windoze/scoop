use super::*;

impl Parser {
    fn context_close(&self) -> Option<usize> {
        if !matches!(self.tokens.get(self.pos + 1)?.kind, TokenKind::LParen) {
            return None;
        }
        let mut depth = 0;
        for (index, token) in self.tokens.iter().enumerate().skip(self.pos + 1) {
            match token.kind {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(index);
                    }
                }
                TokenKind::Eof => break,
                _ => {}
            }
        }
        None
    }

    pub(crate) fn starts_context_scope(&self) -> bool {
        self.context_close().is_some_and(|close| {
            matches!(
                self.tokens.get(close + 1).map(|token| &token.kind),
                Some(TokenKind::LBrace)
            )
        })
    }

    pub(crate) fn starts_context_declaration(&self) -> bool {
        if !matches!(&self.peek().kind, TokenKind::Ident(name) if name == "context") {
            return false;
        }
        let Some(close) = self.context_close() else {
            return false;
        };
        self.tokens
            .get(close + 1)
            .is_some_and(|token| match &token.kind {
                TokenKind::Fun
                | TokenKind::At
                | TokenKind::Suspend
                | TokenKind::Infix
                | TokenKind::Val
                | TokenKind::Var => true,
                TokenKind::Ident(name) => matches!(
                    name.as_str(),
                    "context"
                        | "operator"
                        | "public"
                        | "private"
                        | "protected"
                        | "internal"
                        | "open"
                        | "abstract"
                        | "override"
                        | "final"
                        | "inline"
                        | "tailrec"
                ),
                _ => false,
            })
    }

    pub(super) fn parse_context_scope(&mut self) -> Result<Expr, Diagnostic> {
        let keyword = self.bump();
        self.bump(); // `(`, established by lookahead.
        if matches!(self.peek().kind, TokenKind::RParen) {
            return Err(Diagnostic::at(
                self.peek().span,
                "a context scope requires exactly one binding value",
            ));
        }
        let value = self.parse_expr()?;
        if matches!(self.peek().kind, TokenKind::Comma) {
            return Err(Diagnostic::at(
                self.peek().span,
                "a context scope requires exactly one binding value",
            ));
        }
        self.expect("`)` after context binding value", |kind| {
            matches!(kind, TokenKind::RParen)
        })?;
        let body = self.parse_block()?;
        let span = Span::new(keyword.span.start, body.span.end);
        Ok(Expr::ContextScope {
            value: Box::new(value),
            body,
            span,
        })
    }
}
