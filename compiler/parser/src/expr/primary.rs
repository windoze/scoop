use super::*;

impl Parser {
    pub(super) fn parse_atom(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Suspend => {
                self.pos += 1;
                match self.peek().kind {
                    TokenKind::Fun => self.parse_anonymous_function(true, Some(token.span.start)),
                    TokenKind::LBrace => self.parse_lambda(true, Some(token.span.start)),
                    _ => self.unexpected("`fun` or `{` after `suspend`"),
                }
            }
            TokenKind::Fun => self.parse_anonymous_function(false, None),
            TokenKind::DoubleColon => self.parse_callable_reference(None),
            TokenKind::LBrace => self.parse_lambda(false, None),
            TokenKind::Str(value) => {
                self.pos += 1;
                Ok(Expr::StringLiteral {
                    value,
                    span: token.span,
                })
            }
            TokenKind::Int(value) => {
                self.pos += 1;
                Ok(Expr::IntLiteral {
                    value,
                    span: token.span,
                })
            }
            TokenKind::True => {
                self.pos += 1;
                Ok(Expr::BoolLiteral {
                    value: true,
                    span: token.span,
                })
            }
            TokenKind::False => {
                self.pos += 1;
                Ok(Expr::BoolLiteral {
                    value: false,
                    span: token.span,
                })
            }
            TokenKind::If => self.parse_if_expression(),
            TokenKind::When => self.parse_when_expression(),
            TokenKind::This => {
                self.pos += 1;
                Ok(Expr::This { span: token.span })
            }
            TokenKind::Ident(ref text) if text == "try" => self.parse_try_expression(),
            TokenKind::Ident(text) => {
                self.pos += 1;
                // `Unit` is an ordinary identifier; in expression position
                // it denotes the unit value (spec section 4.3).
                if text == "Unit" {
                    return Ok(Expr::UnitLiteral { span: token.span });
                }
                if text == "super" {
                    return Err(Diagnostic::at(
                        token.span,
                        "`super` calls are not supported yet (milestone M6)",
                    ));
                }
                let ident = Ident {
                    text,
                    span: token.span,
                };
                // `Name(args)` — a call; struct/enum construction shares
                // this syntax, hir-lower tells them apart. A dotted path
                // (`E.V(args)`, `p.m(args)`) is NOT collapsed here: the
                // postfix loop turns it into a method call, and hir-lower
                // resolves enum variant construction from that shape.
                let type_args = self.parse_explicit_call_type_args();
                if matches!(self.peek().kind, TokenKind::LParen) {
                    return self.parse_call(ident, type_args);
                }
                Ok(Expr::Var(ident))
            }
            TokenKind::LParen => self.parse_paren_expr(),
            TokenKind::LBracket => self.parse_array_literal(),
            _ => self.unexpected("expression"),
        }
    }

    /// `[e1, e2, ...]` — an array literal (spec 10.2). The empty `[]`
    /// parses too; HIR rejects it when there is no expected type.
    fn parse_array_literal(&mut self) -> Result<Expr, Diagnostic> {
        let open = self.bump(); // `[`
        let mut elements = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RBracket) {
            loop {
                elements.push(self.parse_expr()?);
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        let close = self.expect("`]`", |k| matches!(k, TokenKind::RBracket))?;
        Ok(Expr::ArrayLiteral {
            elements,
            span: Span::new(open.span.start, close.span.end),
        })
    }

    /// `callee(args...)` — `callee` is already consumed. Struct
    /// construction shares this syntax in M2 (`Point(1, 2)`); hir-lower
    /// tells function calls and struct constructions apart.
    fn parse_call(
        &mut self,
        callee: Ident,
        type_args: Vec<scoop_ast::TypeRef>,
    ) -> Result<Expr, Diagnostic> {
        let (args, end) = self.parse_args()?;
        let span = Span::new(callee.span.start, end);
        Ok(Expr::Call(CallExpr {
            callee,
            type_args,
            args,
            span,
        }))
    }

    /// Parse `<T, ...>` only when it is immediately followed by a call
    /// argument list. The speculative reset keeps ordinary `<` / `>` binary
    /// expressions unchanged.
    pub(super) fn parse_explicit_call_type_args(&mut self) -> Vec<scoop_ast::TypeRef> {
        if !matches!(self.peek().kind, TokenKind::Less) {
            return Vec::new();
        }
        let start = self.pos;
        self.bump();
        let mut type_args = Vec::new();
        loop {
            let Ok(ty) = self.parse_type_ref() else {
                self.pos = start;
                return Vec::new();
            };
            type_args.push(ty);
            if matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        if !matches!(self.peek().kind, TokenKind::Greater) {
            self.pos = start;
            return Vec::new();
        }
        self.bump();
        if !matches!(self.peek().kind, TokenKind::LParen) || self.peek().newline_before {
            self.pos = start;
            return Vec::new();
        }
        type_args
    }

    /// `(arg, ...)` — the `(` is the current token. Shared by calls,
    /// method calls, and base-class constructor delegation. Returns the
    /// arguments and the closing paren's end offset.
    pub(crate) fn parse_args(&mut self) -> Result<(Vec<Expr>, u32), Diagnostic> {
        self.expect("`(`", |k| matches!(k, TokenKind::LParen))?;
        let mut args = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RParen) {
            loop {
                args.push(self.parse_expr()?);
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        let close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
        Ok((args, close.span.end))
    }

    /// `( ... )` disambiguation (spec section 4.3): `()` is the unit
    /// literal, `(e)` is just `e` (no node), `(e,)` a 1-tuple, and
    /// `(e1, e2, ...)` a tuple literal.
    fn parse_paren_expr(&mut self) -> Result<Expr, Diagnostic> {
        let open = self.bump(); // `(`
        if matches!(self.peek().kind, TokenKind::RParen) {
            let close = self.bump();
            return Ok(Expr::UnitLiteral {
                span: Span::new(open.span.start, close.span.end),
            });
        }
        let first = self.parse_expr()?;
        if !matches!(self.peek().kind, TokenKind::Comma) {
            self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
            return Ok(first);
        }
        let mut elements = vec![first];
        while matches!(self.peek().kind, TokenKind::Comma) {
            self.bump();
            if matches!(self.peek().kind, TokenKind::RParen) {
                break; // trailing comma: `(e,)` / `(e1, e2,)`
            }
            elements.push(self.parse_expr()?);
        }
        let close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
        Ok(Expr::TupleLiteral {
            elements,
            span: Span::new(open.span.start, close.span.end),
        })
    }
}
