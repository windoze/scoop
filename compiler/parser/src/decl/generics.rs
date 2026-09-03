use super::*;

impl Parser {
    /// `<T, out U : Interface, ...>`. Variance and upper-bound legality are
    /// intentionally deferred to HIR; the parser preserves the source form.
    pub(super) fn parse_type_params(&mut self) -> Result<Vec<TypeParamDecl>, Diagnostic> {
        let mut type_params = Vec::new();
        if !matches!(self.peek().kind, TokenKind::Less) {
            return Ok(type_params);
        }
        self.bump();
        loop {
            let variance_token = self.peek().clone();
            let variance = match &variance_token.kind {
                TokenKind::In => {
                    self.bump();
                    Variance::In
                }
                TokenKind::Ident(text) if text == "out" => {
                    self.bump();
                    Variance::Out
                }
                _ => Variance::Invariant,
            };
            let name = self.expect_ident("type parameter name")?;
            let inline_bound = if matches!(self.peek().kind, TokenKind::Colon) {
                self.bump();
                Some(self.parse_type_bound()?)
            } else {
                None
            };
            let start = if variance == Variance::Invariant {
                name.span.start
            } else {
                variance_token.span.start
            };
            let end = self.tokens[self.pos.saturating_sub(1)]
                .span
                .end
                .max(name.span.end);
            type_params.push(TypeParamDecl {
                span: Span::new(start, end),
                name,
                variance,
                inline_bound,
            });
            if matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        self.expect("`>`", |k| matches!(k, TokenKind::Greater))?;
        Ok(type_params)
    }

    pub(super) fn parse_type_bound(&mut self) -> Result<TypeBound, Diagnostic> {
        match &self.peek().kind {
            TokenKind::Ident(text) if text == "value" => {
                self.bump();
                Ok(TypeBound::Kind(TypeParamKindBound::Value))
            }
            TokenKind::Ident(text) if text == "ref" => {
                self.bump();
                Ok(TypeBound::Kind(TypeParamKindBound::Ref))
            }
            _ => self.parse_type_ref().map(TypeBound::Upper),
        }
    }

    pub(super) fn parse_where_clause(&mut self) -> Result<Option<WhereClause>, Diagnostic> {
        let TokenKind::Ident(keyword) = &self.peek().kind else {
            return Ok(None);
        };
        if keyword != "where" {
            return Ok(None);
        }
        let start = self.bump().span.start;
        let mut constraints = Vec::new();
        loop {
            let parameter = self.expect_ident("type parameter name in `where` constraint")?;
            self.expect("`:`", |kind| matches!(kind, TokenKind::Colon))?;
            let bound = self.parse_type_bound()?;
            let end = self.tokens[self.pos - 1].span.end;
            constraints.push(TypeConstraint {
                span: Span::new(parameter.span.start, end),
                parameter,
                bound,
            });
            if !matches!(self.peek().kind, TokenKind::Comma) {
                break;
            }
            self.bump();
        }
        let end = constraints
            .last()
            .expect("a where clause always has one constraint")
            .span
            .end;
        Ok(Some(WhereClause {
            constraints,
            span: Span::new(start, end),
        }))
    }
}
