use super::*;
use scoop_ast::TypeConstraintSubject;

mod requirements;

impl Parser {
    /// `<T, U : Interface, ...>`. Nominal type parameters are permanently
    /// invariant, so declaration-site variance is rejected at the grammar
    /// boundary and never enters the AST.
    pub(super) fn parse_type_params(&mut self) -> Result<Vec<TypeParamDecl>, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::TypeParameterList, |parser| {
            parser.parse_type_params_inner()
        })
    }

    fn parse_type_params_inner(&mut self) -> Result<Vec<TypeParamDecl>, Diagnostic> {
        if !matches!(self.peek().kind, TokenKind::Less) {
            return Ok(Vec::new());
        }
        self.bump();
        let (parameters, _) = self
            .parse_delimited_items(
                crate::parser::ListKind::TypeParameters,
                Self::parse_type_parameter,
            )
            .into_parts();
        Ok(parameters)
    }

    fn parse_type_parameter(&mut self) -> Result<TypeParamDecl, Diagnostic> {
        self.with_node(
            scoop_syntax::SyntaxKind::TypeParameter,
            Self::parse_type_parameter_inner,
        )
    }

    fn parse_type_parameter_inner(&mut self) -> Result<TypeParamDecl, Diagnostic> {
        let doc_start = self.pos;
        while matches!(&self.peek().kind, TokenKind::Ident(text) if text == "reified")
            && matches!(
                self.tokens[self.pos + 1].kind,
                TokenKind::Ident(_) | TokenKind::EscapedIdent(_) | TokenKind::In
            )
        {
            self.bump();
        }
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
        let documented = TypeParamDecl {
            span: Span::new(name.span.start, end),
            name,
            inline_bound,
        };
        self.attach_documentation(doc_start, documented.span);
        Ok(documented)
    }

    pub(super) fn parse_type_bound(&mut self) -> Result<TypeBound, Diagnostic> {
        match &self.peek().kind {
            TokenKind::Bang => {
                self.bump();
                self.parse_type_ref().map(TypeBound::ExcludedInterface)
            }
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
        self.with_node(scoop_syntax::SyntaxKind::WhereClause, |parser| {
            parser.parse_where_clause_inner()
        })
    }

    fn parse_where_clause_inner(&mut self) -> Result<Option<WhereClause>, Diagnostic> {
        let TokenKind::Ident(keyword) = &self.peek().kind else {
            return Ok(None);
        };
        if keyword != "where" {
            return Ok(None);
        }
        let start = self.bump().span.start;
        let mut constraints = Vec::new();
        let mut requirements = Vec::new();
        loop {
            if matches!(&self.peek().kind, TokenKind::Ident(text) if text == "requires")
                && matches!(
                    self.tokens[self.pos + 1].kind,
                    TokenKind::LParen | TokenKind::LBrace
                )
            {
                requirements.push(self.parse_member_requirements()?);
            } else {
                constraints.push(self.parse_type_constraint()?);
            }
            if !matches!(self.peek().kind, TokenKind::Comma) {
                break;
            }
            self.bump();
        }
        let end = self.tokens[self.pos - 1].span.end;
        Ok(Some(WhereClause {
            constraints,
            requirements,
            span: Span::new(start, end),
        }))
    }

    fn parse_type_constraint(&mut self) -> Result<TypeConstraint, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::TypeConstraint, |parser| {
            parser.parse_type_constraint_inner()
        })
    }

    fn parse_type_constraint_inner(&mut self) -> Result<TypeConstraint, Diagnostic> {
        let parameter = self.expect_ident("type parameter name in `where` constraint")?;
        let start = parameter.span.start;
        let subject = if matches!(self.peek().kind, TokenKind::Dot) {
            self.bump();
            let member = self.expect_ident("`Companion` in projected constraint")?;
            if member.text != "Companion" {
                return Err(Diagnostic::at(
                    member.span,
                    "a dependent constraint only supports the `Companion` projection",
                ));
            }
            TypeConstraintSubject::Companion(parameter)
        } else {
            TypeConstraintSubject::Parameter(parameter)
        };
        self.expect("`:`", |kind| matches!(kind, TokenKind::Colon))?;
        let bound = self.parse_type_bound()?;
        Ok(TypeConstraint {
            span: Span::new(start, self.tokens[self.pos - 1].span.end),
            subject,
            bound,
        })
    }
}
