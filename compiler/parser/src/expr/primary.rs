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
            TokenKind::Char(value) => {
                self.pos += 1;
                Ok(Expr::CharLiteral {
                    value,
                    span: token.span,
                })
            }
            TokenKind::Str(value) => {
                self.pos += 1;
                Ok(Expr::StringLiteral {
                    value,
                    span: token.span,
                })
            }
            TokenKind::Int(lexeme) => {
                self.pos += 1;
                Ok(Expr::IntLiteral(lexeme.with_span(token.span)))
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
            TokenKind::Break => Err(loop_jump_expression_diagnostic("break", token.span)),
            TokenKind::Continue => Err(loop_jump_expression_diagnostic("continue", token.span)),
            TokenKind::Ident(ref text) if text == "try" => self.parse_try_expression(),
            TokenKind::Ident(text) => {
                self.pos += 1;
                // `Unit` is an ordinary identifier; in expression position
                // it denotes the unit value (spec section 4.3).
                if text == "Unit" {
                    return Ok(Expr::UnitLiteral { span: token.span });
                }
                if text == "super" {
                    return self.parse_super_method_call(token.span);
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
                let type_args = self.parse_explicit_call_type_args()?;
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

    fn parse_super_method_call(&mut self, super_span: Span) -> Result<Expr, Diagnostic> {
        if matches!(self.peek().kind, TokenKind::QuestionDot) {
            return Err(Diagnostic::at(
                self.peek().span,
                "safe navigation is not allowed on `super`",
            ));
        }
        if matches!(self.peek().kind, TokenKind::DoubleColon) {
            return Err(Diagnostic::at(
                self.peek().span,
                "callable references cannot target `super`",
            ));
        }
        if matches!(self.peek().kind, TokenKind::Less) {
            return self.parse_qualified_interface_super(super_span);
        }
        if !matches!(self.peek().kind, TokenKind::Dot) {
            return Err(Diagnostic::at(
                super_span,
                "`super` is only valid as the receiver of a direct base method call",
            ));
        }
        self.bump();
        let name = self.expect_ident("base method name after `super.`")?;
        let type_args = self.parse_explicit_call_type_args()?;
        if !matches!(self.peek().kind, TokenKind::LParen) {
            return Err(Diagnostic::at(
                name.span,
                "`super` only supports method calls, not field access",
            ));
        }
        let (args, end) = self.parse_args()?;
        Ok(Expr::SuperMethodCall {
            super_span,
            name,
            type_args,
            args,
            span: Span::new(super_span.start, end),
        })
    }

    fn parse_qualified_interface_super(&mut self, super_span: Span) -> Result<Expr, Diagnostic> {
        self.bump(); // `<`
        let qualifier = self.parse_type_ref()?;
        self.expect("`>` after qualified `super` interface", |kind| {
            matches!(kind, TokenKind::Greater)
        })?;
        self.expect("`.` after qualified `super` interface", |kind| {
            matches!(kind, TokenKind::Dot)
        })?;
        let name = self.expect_ident("interface member name after qualified `super`")?;
        let type_args = self.parse_explicit_call_type_args()?;
        if matches!(self.peek().kind, TokenKind::LParen) {
            let (args, end) = self.parse_args()?;
            return Ok(Expr::QualifiedInterfaceSuperMethodCall {
                super_span,
                qualifier,
                name,
                type_args,
                args,
                span: Span::new(super_span.start, end),
            });
        }
        if !type_args.is_empty() {
            return Err(Diagnostic::at(
                name.span,
                "qualified `super` member type arguments require a method call",
            ));
        }
        Ok(Expr::QualifiedInterfaceSuperAccess {
            super_span,
            qualifier,
            span: Span::new(super_span.start, name.span.end),
            name,
        })
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
        type_args: Vec<scoop_ast::CallTypeArgument>,
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
    pub(super) fn parse_explicit_call_type_args(
        &mut self,
    ) -> Result<Vec<scoop_ast::CallTypeArgument>, Diagnostic> {
        if !matches!(self.peek().kind, TokenKind::Less) {
            return Ok(Vec::new());
        }
        let start = self.pos;
        self.bump();
        let mut type_args = Vec::new();
        let mut saw_infer = false;
        loop {
            let argument = if matches!(&self.peek().kind, TokenKind::Ident(text) if text == "_") {
                saw_infer = true;
                let token = self.bump();
                scoop_ast::CallTypeArgument::Infer { span: token.span }
            } else {
                let ty = match self.parse_type_ref() {
                    Ok(ty) => ty,
                    Err(error)
                        if saw_infer
                            || error.message
                                == "`_` is only allowed in call type argument lists" =>
                    {
                        return Err(error);
                    }
                    Err(_) => {
                        self.pos = start;
                        return Ok(Vec::new());
                    }
                };
                scoop_ast::CallTypeArgument::Explicit(ty)
            };
            type_args.push(argument);
            if matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        if !matches!(self.peek().kind, TokenKind::Greater) {
            if saw_infer {
                return self.unexpected("`>` after call type argument list");
            }
            self.pos = start;
            return Ok(Vec::new());
        }
        self.bump();
        if !matches!(self.peek().kind, TokenKind::LParen) || self.peek().newline_before {
            if saw_infer {
                return Err(Diagnostic::at(
                    self.tokens[start].span,
                    "`_` is only allowed in a type argument list immediately followed by a call",
                ));
            }
            self.pos = start;
            return Ok(Vec::new());
        }
        Ok(type_args)
    }

    /// `(arg, ...)` — the `(` is the current token. Shared by calls,
    /// method calls, and base-class constructor delegation. Returns the
    /// arguments and the closing paren's end offset.
    pub(crate) fn parse_args(&mut self) -> Result<(Vec<CallArgument>, u32), Diagnostic> {
        self.expect("`(`", |k| matches!(k, TokenKind::LParen))?;
        let mut args = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RParen) {
            loop {
                args.push(self.parse_call_argument()?);
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

    fn parse_call_argument(&mut self) -> Result<CallArgument, Diagnostic> {
        let start = self.peek().span.start;
        let name = if let TokenKind::Ident(_) = &self.peek().kind
            && matches!(
                self.tokens.get(self.pos + 1).map(|token| &token.kind),
                Some(TokenKind::Equal)
            ) {
            let name = self.expect_ident("argument name")?;
            self.bump(); // `=` verified above
            CallArgumentName::Named(name)
        } else {
            CallArgumentName::Positional
        };
        let spread = if matches!(self.peek().kind, TokenKind::Star) {
            SpreadSyntax::Spread(self.bump().span)
        } else {
            SpreadSyntax::Plain
        };
        let expression = self.parse_expr()?;
        Ok(CallArgument {
            span: Span::new(start, expression.span().end),
            name,
            spread,
            expression,
        })
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
