use super::*;

impl Parser {
    /// Postfix operators share the highest precedence tier and chain left
    /// to right: `.name` / `._n`, `?.name`, `!!`, and `[index]`.
    pub(crate) fn parse_postfix(&mut self) -> Result<Expr, Diagnostic> {
        let mut grouped = matches!(self.peek().kind, TokenKind::LParen);
        let mut receiver = self.parse_atom()?;
        loop {
            match self.peek().kind {
                TokenKind::Dot => {
                    self.bump();
                    receiver = if matches!(self.peek().kind, TokenKind::LBrace) {
                        self.parse_copy_update(receiver)?
                    } else {
                        self.parse_field_access(receiver, false)?
                    };
                }
                TokenKind::QuestionDot => {
                    let navigation = self.bump();
                    if matches!(self.peek().kind, TokenKind::LBrace) {
                        return Err(Diagnostic::at(
                            navigation.span,
                            "copy update does not support safe navigation; unwrap the Option with `when` first",
                        ));
                    }
                    receiver = self.parse_field_access(receiver, true)?;
                }
                TokenKind::DoubleColon => {
                    receiver = self.parse_callable_reference(Some(receiver))?;
                }
                TokenKind::LParen
                    if !self.peek().newline_before && matches!(receiver, Expr::This { .. }) =>
                {
                    return Err(Diagnostic::at(
                        receiver.span(),
                        "`this(...)` is only valid in a constructor delegation clause",
                    ));
                }
                TokenKind::LParen if !self.peek().newline_before => {
                    let start = receiver.span().start;
                    let (args, end) = self.parse_call_inputs()?;
                    receiver = Expr::Invoke {
                        callee: Box::new(receiver),
                        type_args: Vec::new(),
                        args,
                        span: Span::new(start, end),
                    };
                }
                TokenKind::LBrace | TokenKind::Suspend
                    if self.starts_trailing_lambda()
                        && !matches!(receiver, Expr::Return { .. } | Expr::Throw { .. }) =>
                {
                    receiver = self.parse_trailing_lambda(receiver, grouped)?;
                }
                // A `[` immediately after the receiver (their spans touch)
                // is subscript postfix; otherwise it starts a new array
                // literal expression — e.g. a `[...]` statement on the
                // next line (DESIGN.md section 2.1).
                TokenKind::LBracket if self.peek().span.start == receiver.span().end => {
                    receiver = self.parse_index(receiver)?;
                }
                TokenKind::Less if !self.peek().newline_before => {
                    if let Some(qualifier) = self.parse_applied_qualifier(&receiver)? {
                        receiver = Expr::TypeQualifier(qualifier);
                        continue;
                    }
                    let type_args = self.parse_explicit_call_type_args()?;
                    if type_args.is_empty() {
                        break;
                    }
                    let start = receiver.span().start;
                    let (args, end) = self.parse_call_inputs()?;
                    receiver = Expr::Invoke {
                        callee: Box::new(receiver),
                        type_args,
                        args,
                        span: Span::new(start, end),
                    };
                }
                // `a!!` lexes as two adjacent `Bang` tokens — a single
                // `!!` token would break the double negation `!!flag`,
                // which is valid prefix syntax since M1.
                TokenKind::Bang if self.at_null_assert() => {
                    self.bump(); // first `!`
                    let second = self.bump(); // second `!`
                    receiver = Expr::NullAssert {
                        span: Span::new(receiver.span().start, second.span.end),
                        operand: Box::new(receiver),
                    };
                }
                TokenKind::PlusPlus | TokenKind::MinusMinus => {
                    let token = self.bump();
                    let start = receiver.span().start;
                    let place = Self::expr_into_place(receiver, "update operand")?;
                    receiver = Expr::Update {
                        place,
                        op: if matches!(token.kind, TokenKind::PlusPlus) {
                            UpdateOp::Increment
                        } else {
                            UpdateOp::Decrement
                        },
                        notation: UpdateNotation::Postfix,
                        span: Span::new(start, token.span.end),
                    };
                }
                _ => break,
            }
            grouped = false;
        }
        Ok(receiver)
    }

    /// True when the current `!` is immediately followed by another `!`
    /// with no trivia in between (their spans touch).
    fn at_null_assert(&self) -> bool {
        let Some(next) = self.tokens.get(self.pos + 1) else {
            return false;
        };
        matches!(next.kind, TokenKind::Bang) && next.span.start == self.peek().span.end
    }

    /// `.name` / `.name(args)` / `._n` (or `?.name` when `safe`). The dot
    /// token is already consumed. A name directly followed by `(` is a
    /// method call; otherwise the selector is a field. `?.` accepts only
    /// named selectors and, in M6, no method calls (DESIGN.md section 6).
    fn parse_field_access(&mut self, receiver: Expr, safe: bool) -> Result<Expr, Diagnostic> {
        let token = self.peek().clone();
        let TokenKind::Ident(text) = token.kind else {
            return self.unexpected("field name or tuple index");
        };
        if let Some(index) = tuple_index(&text) {
            if safe {
                return self.unexpected("field name");
            }
            self.pos += 1;
            return Ok(Expr::FieldAccess(FieldAccess {
                span: Span::new(receiver.span().start, token.span.end),
                receiver: Box::new(receiver),
                selector: FieldSelector::Index(index, token.span),
                navigation: Navigation::Direct,
            }));
        }
        self.pos += 1;
        let name = Ident {
            text,
            span: token.span,
        };
        let type_args = self.parse_explicit_call_type_args()?;
        if matches!(self.peek().kind, TokenKind::LParen) || self.starts_trailing_lambda() {
            let (args, end) = self.parse_call_inputs()?;
            return Ok(Expr::MethodCall {
                span: Span::new(receiver.span().start, end),
                receiver: Box::new(receiver),
                name,
                navigation: if safe {
                    Navigation::Safe
                } else {
                    Navigation::Direct
                },
                type_args,
                args,
            });
        }
        Ok(Expr::FieldAccess(FieldAccess {
            span: Span::new(receiver.span().start, token.span.end),
            receiver: Box::new(receiver),
            selector: FieldSelector::Name(name),
            navigation: if safe {
                Navigation::Safe
            } else {
                Navigation::Direct
            },
        }))
    }
}
