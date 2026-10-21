use super::*;

impl Parser {
    pub(super) fn parse_atom(&mut self) -> Result<Expr, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::Atom, |parser| {
            parser.parse_atom_inner()
        })
    }

    fn parse_atom_inner(&mut self) -> Result<Expr, Diagnostic> {
        if self.starts_callable_label() {
            return self.parse_labeled_callable();
        }
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Return => self.parse_return_expression(),
            TokenKind::Ident(ref text)
                if text == "object"
                    && self.tokens.get(self.pos + 1).is_some_and(|token| {
                        matches!(token.kind, TokenKind::LBrace | TokenKind::Colon)
                    }) =>
            {
                self.parse_anonymous_object()
            }
            TokenKind::Ident(ref text) if text == "throw" => self.parse_throw_expression(),
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
            TokenKind::FStringStart => self.parse_interpolated_string(),
            TokenKind::Float(literal) => {
                self.pos += 1;
                Ok(Expr::FloatLiteral(literal))
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
                let label =
                    if matches!(self.peek().kind, TokenKind::At) && !self.peek().newline_before {
                        self.bump();
                        Some(self.expect_ident("receiver label after `this@`")?)
                    } else {
                        None
                    };
                let end = label
                    .as_ref()
                    .map_or(token.span.end, |label| label.span.end);
                Ok(Expr::This {
                    label,
                    span: Span::new(token.span.start, end),
                })
            }
            TokenKind::Break | TokenKind::Continue => self.parse_loop_jump_expression(),
            TokenKind::Ident(ref text) if text == "try" => self.parse_try_expression(),
            TokenKind::Ident(ref text) if text == "context" && self.starts_context_scope() => {
                self.parse_context_scope()
            }
            TokenKind::Ident(ref text) if text == "super" => {
                self.pos += 1;
                self.parse_super_method_call(token.span)
            }
            TokenKind::Ident(text) | TokenKind::EscapedIdent(text) => {
                self.pos += 1;
                // `Unit` is an ordinary identifier; in expression position
                // it denotes the unit value (spec section 4.3).
                if text == "Unit" {
                    return Ok(Expr::UnitLiteral { span: token.span });
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
                if matches!(self.peek().kind, TokenKind::LParen) || self.starts_trailing_lambda() {
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
        if !matches!(self.peek().kind, TokenKind::LParen) && !self.starts_trailing_lambda() {
            return Err(Diagnostic::at(
                name.span,
                "`super` only supports method calls, not field access",
            ));
        }
        let (args, end) = self.parse_call_inputs()?;
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
        if matches!(self.peek().kind, TokenKind::LParen) || self.starts_trailing_lambda() {
            let (args, end) = self.parse_call_inputs()?;
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
        self.with_node(scoop_syntax::SyntaxKind::ArrayExpr, |parser| {
            parser.parse_array_literal_inner()
        })
    }

    fn parse_array_literal_inner(&mut self) -> Result<Expr, Diagnostic> {
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
        let (args, end) = self.parse_call_inputs()?;
        let span = Span::new(callee.span.start, end);
        Ok(Expr::Call(CallExpr {
            callee,
            type_args,
            args,
            span,
        }))
    }

    /// Parse `<T, ...>` only when followed by a call argument list or
    /// an external lambda. The speculative reset keeps ordinary `<` / `>` binary
    /// expressions unchanged.
    pub(crate) fn parse_explicit_call_type_args(
        &mut self,
    ) -> Result<Vec<scoop_ast::CallTypeArgument>, Diagnostic> {
        if !matches!(self.peek().kind, TokenKind::Less) {
            return Ok(Vec::new());
        }
        let start = self.checkpoint();
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
                    Ok(ty) if saw_infer || !self.has_errors_since(start) => ty,
                    Ok(_) => {
                        self.restore(start);
                        return Ok(Vec::new());
                    }
                    Err(error)
                        if saw_infer
                            || error.message
                                == "`_` is only allowed in call type argument lists" =>
                    {
                        return Err(error);
                    }
                    Err(_) => {
                        self.restore(start);
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
            self.restore(start);
            return Ok(Vec::new());
        }
        self.bump();
        if !(matches!(self.peek().kind, TokenKind::LParen) && !self.peek().newline_before)
            && !self.starts_trailing_lambda()
        {
            if saw_infer {
                return Err(Diagnostic::at(
                    self.tokens[start.position].span,
                    "`_` is only allowed in a type argument list immediately followed by a call",
                ));
            }
            self.restore(start);
            return Ok(Vec::new());
        }
        Ok(type_args)
    }

    /// `(arg, ...)` — the `(` is the current token. Shared by calls,
    /// method calls, and base-class constructor delegation. Returns the
    /// arguments and the closing paren's end offset.
    pub(crate) fn parse_args(&mut self) -> Result<(Vec<CallArgument>, u32), Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::ArgumentList, |parser| {
            parser.parse_args_inner()
        })
    }

    fn parse_args_inner(&mut self) -> Result<(Vec<CallArgument>, u32), Diagnostic> {
        self.expect("`(`", |k| matches!(k, TokenKind::LParen))?;
        Ok(self.parse_delimited_items(
            crate::parser::ListKind::CallArguments,
            Self::parse_call_argument,
        ))
    }

    fn parse_call_argument(&mut self) -> Result<CallArgument, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::Argument, |parser| {
            parser.parse_call_argument_inner()
        })
    }

    fn parse_call_argument_inner(&mut self) -> Result<CallArgument, Diagnostic> {
        let start = self.peek().span.start;
        let name = if let TokenKind::Ident(_) | TokenKind::EscapedIdent(_) = &self.peek().kind
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
        self.with_node(scoop_syntax::SyntaxKind::ParenExpr, |parser| {
            parser.parse_paren_expr_inner()
        })
    }

    fn parse_paren_expr_inner(&mut self) -> Result<Expr, Diagnostic> {
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
