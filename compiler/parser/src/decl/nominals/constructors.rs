use super::*;

impl Parser {
    pub(super) fn parse_init_block(&mut self) -> Result<InitBlockDecl, Diagnostic> {
        let keyword = self.bump();
        if !matches!(self.peek().kind, TokenKind::LBrace) {
            return Err(Diagnostic::at(
                self.peek().span,
                "`init` must be followed by a block",
            ));
        }
        let body = self.parse_block()?;
        Ok(InitBlockDecl {
            span: Span::new(keyword.span.start, body.span.end),
            body,
        })
    }

    pub(super) fn parse_secondary_constructor(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: VisibilitySyntax,
    ) -> Result<SecondaryConstructorDecl, Diagnostic> {
        let keyword = self.bump();
        if matches!(self.peek().kind, TokenKind::Less) {
            return Err(Diagnostic::at(
                self.peek().span,
                "secondary constructors cannot declare type parameters",
            ));
        }
        self.expect("`(` after `constructor`", |kind| {
            matches!(kind, TokenKind::LParen)
        })?;
        let mut params = Vec::new();
        let mut vararg_span = None;
        if !matches!(self.peek().kind, TokenKind::RParen) {
            loop {
                let modifier_span = self.parse_vararg_modifier(&mut vararg_span)?;
                if matches!(self.peek().kind, TokenKind::Val | TokenKind::Var) {
                    return Err(Diagnostic::at(
                        self.peek().span,
                        "secondary constructor parameters cannot declare `val` or `var` properties",
                    ));
                }
                let name = self.expect_ident("constructor parameter name")?;
                self.expect("`:`", |kind| matches!(kind, TokenKind::Colon))?;
                let ty = self.parse_type_ref()?;
                let (syntax, end) = self.parse_parameter_syntax(modifier_span, ty.span.end)?;
                params.push(Param {
                    span: Span::new(
                        modifier_span.map_or(name.span.start, |span| span.start),
                        end,
                    ),
                    name,
                    ty,
                    syntax,
                });
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        self.expect("`)`", |kind| matches!(kind, TokenKind::RParen))?;
        let delegation = if matches!(self.peek().kind, TokenKind::Colon) {
            self.bump();
            Some(self.parse_constructor_delegation()?)
        } else {
            None
        };
        if !matches!(self.peek().kind, TokenKind::LBrace) {
            return Err(Diagnostic::at(
                self.peek().span,
                "secondary constructors require a block body",
            ));
        }
        let body = self.parse_block()?;
        let start = annotations
            .first()
            .map(|annotation| annotation.span.start)
            .into_iter()
            .chain(match visibility {
                VisibilitySyntax::Explicit { span, .. } => Some(span.start),
                VisibilitySyntax::Omitted => None,
            })
            .min()
            .unwrap_or(keyword.span.start);
        Ok(SecondaryConstructorDecl {
            annotations,
            visibility,
            params,
            delegation,
            span: Span::new(start, body.span.end),
            body,
        })
    }

    fn parse_constructor_delegation(&mut self) -> Result<ConstructorDelegation, Diagnostic> {
        let target = self.peek().clone();
        let is_this = match &target.kind {
            TokenKind::This => true,
            TokenKind::Ident(text) if text == "super" => false,
            _ => {
                return Err(Diagnostic::at(
                    target.span,
                    "constructor delegation target must be `this` or `super`",
                ));
            }
        };
        self.bump();
        if !matches!(self.peek().kind, TokenKind::LParen) {
            return Err(Diagnostic::at(
                self.peek().span,
                "constructor delegation requires an argument list",
            ));
        }
        let (arguments, end) = self.parse_args()?;
        let span = Span::new(target.span.start, end);
        Ok(if is_this {
            ConstructorDelegation::This {
                target_span: target.span,
                arguments,
                span,
            }
        } else {
            ConstructorDelegation::Super {
                target_span: target.span,
                arguments,
                span,
            }
        })
    }
}
