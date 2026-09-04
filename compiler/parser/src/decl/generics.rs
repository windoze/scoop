use super::*;

impl Parser {
    /// `<T, U : Interface, ...>`. Nominal type parameters are permanently
    /// invariant, so declaration-site variance is rejected at the grammar
    /// boundary and never enters the AST.
    pub(super) fn parse_type_params(&mut self) -> Result<Vec<TypeParamDecl>, Diagnostic> {
        let mut type_params = Vec::new();
        if !matches!(self.peek().kind, TokenKind::Less) {
            return Ok(type_params);
        }
        self.bump();
        loop {
            if matches!(self.peek().kind, TokenKind::In)
                || matches!(&self.peek().kind, TokenKind::Ident(text) if text == "out")
            {
                let token = self.bump();
                return Err(Diagnostic::at(
                    token.span,
                    "nominal type parameters are invariant; declaration-site `in`/`out` is not supported",
                ));
            }
            let name = self.expect_ident("type parameter name")?;
            let inline_bound = if matches!(self.peek().kind, TokenKind::Colon) {
                self.bump();
                Some(self.parse_type_bound()?)
            } else {
                None
            };
            let end = self.tokens[self.pos.saturating_sub(1)]
                .span
                .end
                .max(name.span.end);
            type_params.push(TypeParamDecl {
                span: Span::new(name.span.start, end),
                name,
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
