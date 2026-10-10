use super::*;

impl Parser {
    /// `receiver.{ field: value, ... }`. The direct dot has already been
    /// consumed and `{` is current. This list deliberately has its own
    /// grammar so it cannot be mistaken for a lambda or statement block.
    pub(super) fn parse_copy_update(&mut self, base: Expr) -> Result<Expr, Diagnostic> {
        self.bump();
        if matches!(self.peek().kind, TokenKind::RBrace) {
            return Err(Diagnostic::at(
                self.peek().span,
                "copy update field list must not be empty",
            ));
        }

        let mut fields = Vec::new();
        loop {
            if matches!(self.peek().kind, TokenKind::DotDot) {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "copy update field list does not allow rest entries",
                ));
            }
            let field = self.expect_ident("copy update field name")?;
            if matches!(self.peek().kind, TokenKind::Dot | TokenKind::QuestionDot) {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "copy update fields must be direct names, not nested paths",
                ));
            }
            self.expect("`:` after copy update field name", |kind| {
                matches!(kind, TokenKind::Colon)
            })?;
            let statement_only = matches!(
                self.peek().kind,
                TokenKind::Val
                    | TokenKind::Var
                    | TokenKind::While
                    | TokenKind::For
                    | TokenKind::Break
                    | TokenKind::Continue
            );
            if statement_only {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "copy update field values must be expressions, not statements",
                ));
            }
            let value = self.parse_expr()?;
            let span = Span::new(field.span.start, value.span().end);
            fields.push(FieldUpdate { field, value, span });

            if matches!(self.peek().kind, TokenKind::RBrace) {
                break;
            }
            self.expect("`,` or `}` after copy update field", |kind| {
                matches!(kind, TokenKind::Comma)
            })?;
            if matches!(self.peek().kind, TokenKind::RBrace) {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "copy update field list does not allow a trailing comma",
                ));
            }
        }
        let close = self.expect("`}`", |kind| matches!(kind, TokenKind::RBrace))?;
        let mut fields = fields.into_iter();
        let first = fields
            .next()
            .expect("the parser rejected an empty copy-update list");
        let fields = NonEmptyVec::new(first, fields.collect());
        Ok(Expr::CopyUpdate {
            span: Span::new(base.span().start, close.span.end),
            base: Box::new(base),
            fields,
        })
    }

    /// `receiver[index]` — the `[` is the current token. A `:` after the
    /// index expression is a slice, out of the M5 subset.
    pub(super) fn parse_index(&mut self, receiver: Expr) -> Result<Expr, Diagnostic> {
        self.bump(); // `[`
        let first = self.parse_expr()?;
        if matches!(self.peek().kind, TokenKind::Colon) {
            return Err(Diagnostic::at(
                self.peek().span,
                "array slices are not supported yet (milestone M5)",
            ));
        }
        let mut rest = Vec::new();
        while matches!(self.peek().kind, TokenKind::Comma) {
            self.bump();
            if matches!(self.peek().kind, TokenKind::RBracket) {
                return self.unexpected("index expression after `,`");
            }
            rest.push(self.parse_expr()?);
        }
        let close = self.expect("`]`", |k| matches!(k, TokenKind::RBracket))?;
        Ok(Expr::Index {
            span: Span::new(receiver.span().start, close.span.end),
            receiver: Box::new(receiver),
            indices: NonEmptyVec::new(first, rest),
        })
    }

    pub(crate) fn expr_into_place(expr: Expr, context: &str) -> Result<PlaceExpr, Diagnostic> {
        let span = expr.span();
        match expr {
            Expr::Var(name) => Ok(PlaceExpr::Name(name)),
            Expr::FieldAccess(access) => match (access.navigation, access.selector) {
                (Navigation::Direct, FieldSelector::Name(name)) => Ok(PlaceExpr::Field {
                    receiver: access.receiver,
                    name,
                    span: access.span,
                }),
                (Navigation::Safe, _) if context == "assignment target" => Err(Diagnostic::at(
                    span,
                    "assignments through `?.` are not allowed",
                )),
                _ => Err(Diagnostic::at(
                    span,
                    format!("{context} must be an assignable place"),
                )),
            },
            Expr::Index {
                receiver,
                indices,
                span,
            } => Ok(PlaceExpr::Index {
                receiver,
                indices,
                span,
            }),
            Expr::QualifiedInterfaceSuperAccess {
                qualifier,
                name,
                span,
                ..
            } => Ok(PlaceExpr::QualifiedInterfaceSuperProperty {
                qualifier,
                name,
                span,
            }),
            _ => Err(Diagnostic::at(
                span,
                format!("{context} must be an assignable place"),
            )),
        }
    }
}
