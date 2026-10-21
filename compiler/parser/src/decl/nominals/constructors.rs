use super::*;

impl Parser {
    pub(super) fn parse_primary_constructor(&mut self) -> Result<ClassConstructorDecl, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::PrimaryConstructor, |parser| {
            parser.parse_primary_constructor_inner()
        })
    }

    fn parse_primary_constructor_inner(&mut self) -> Result<ClassConstructorDecl, Diagnostic> {
        let constructor_doc_start = self.checkpoint();
        let starts_new_line = self.peek().newline_before;
        let mut constructor_annotations = Vec::new();
        let mut constructor_visibility = VisibilitySyntax::Omitted;
        loop {
            if matches!(self.peek().kind, TokenKind::At) {
                constructor_annotations.extend(self.parse_annotations()?);
                continue;
            }
            if visibility_token(&self.peek().kind).is_some() {
                if !matches!(constructor_visibility, VisibilitySyntax::Omitted) {
                    return Err(Diagnostic::at(
                        self.peek().span,
                        "a constructor may have only one visibility modifier",
                    ));
                }
                constructor_visibility = self.parse_visibility()?;
                continue;
            }
            break;
        }
        let explicit_constructor = if matches!(&self.peek().kind, TokenKind::Ident(text) if text == "constructor")
        {
            Some(self.bump())
        } else {
            None
        };
        // A bodyless class may be followed by another declaration. Its
        // leading annotations and visibility are not constructor modifiers.
        if explicit_constructor.is_none()
            && starts_new_line
            && !matches!(self.peek().kind, TokenKind::LParen)
        {
            self.restore(constructor_doc_start);
            return Ok(ClassConstructorDecl::Omitted);
        }
        if (!constructor_annotations.is_empty()
            || !matches!(constructor_visibility, VisibilitySyntax::Omitted))
            && explicit_constructor.is_none()
        {
            return Err(Diagnostic::at(
                self.peek().span,
                "primary constructor annotations or visibility require the `constructor` keyword",
            ));
        }
        if explicit_constructor.is_some() && !matches!(self.peek().kind, TokenKind::LParen) {
            return Err(Diagnostic::at(
                self.peek().span,
                "an explicit primary constructor requires a parameter list",
            ));
        }
        let constructor = if matches!(self.peek().kind, TokenKind::LParen) {
            let open = self.bump();
            let mut parameters = Vec::new();
            let mut vararg_span = None;
            if !matches!(self.peek().kind, TokenKind::RParen) {
                loop {
                    let doc_start = self.pos;
                    let annotations = self.parse_annotations()?;
                    let modifier_span = self.parse_vararg_modifier(&mut vararg_span)?;
                    let member_visibility = self.parse_visibility()?;
                    let override_span = if matches!(&self.peek().kind, TokenKind::Ident(text) if text == "override")
                    {
                        Some(self.bump().span)
                    } else {
                        None
                    };
                    let first = self.peek().clone();
                    let property = match first.kind {
                        TokenKind::Val => {
                            self.pos += 1;
                            PrimaryParameterProperty::Val
                        }
                        TokenKind::Var => {
                            self.pos += 1;
                            PrimaryParameterProperty::Var
                        }
                        _ => PrimaryParameterProperty::Plain,
                    };
                    if !property.is_property()
                        && (!matches!(member_visibility, VisibilitySyntax::Omitted)
                            || override_span.is_some())
                    {
                        return Err(Diagnostic::at(
                            first.span,
                            "primary constructor member visibility and `override` require `val` or `var`",
                        ));
                    }
                    let parameter_name = self.expect_ident("primary constructor parameter name")?;
                    self.expect("`:`", |k| matches!(k, TokenKind::Colon))?;
                    let ty = self.parse_type_ref()?;
                    let (syntax, end) = self.parse_parameter_syntax(modifier_span, ty.span.end)?;
                    let documented = PrimaryClassParameter {
                        annotations,
                        property,
                        member_visibility: property.is_property().then_some(member_visibility),
                        is_override: override_span.is_some(),
                        span: Span::new(
                            modifier_span
                                .map(|span| span.start)
                                .or(match member_visibility {
                                    VisibilitySyntax::Explicit { span, .. } => Some(span.start),
                                    VisibilitySyntax::Omitted => None,
                                })
                                .or_else(|| override_span.map(|span| span.start))
                                .unwrap_or(first.span.start),
                            end,
                        ),
                        name: parameter_name,
                        ty,
                        syntax,
                    };
                    self.attach_documentation(doc_start, documented.span);
                    parameters.push(documented);
                    if matches!(self.peek().kind, TokenKind::Comma) {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            let end = self
                .expect("`)`", |k| matches!(k, TokenKind::RParen))?
                .span
                .end;
            let constructor_start = constructor_annotations
                .first()
                .map(|annotation| annotation.span.start)
                .into_iter()
                .chain(match constructor_visibility {
                    VisibilitySyntax::Explicit { span, .. } => Some(span.start),
                    VisibilitySyntax::Omitted => None,
                })
                .chain(
                    explicit_constructor
                        .as_ref()
                        .map(|keyword| keyword.span.start),
                )
                .min()
                .unwrap_or(open.span.start);
            self.attach_documentation(
                constructor_doc_start.position,
                Span::new(constructor_start, end),
            );
            ClassConstructorDecl::Declared(PrimaryConstructorDecl {
                annotations: constructor_annotations,
                visibility: constructor_visibility,
                parameters,
                span: Span::new(constructor_start, end),
            })
        } else {
            ClassConstructorDecl::Omitted
        };
        Ok(constructor)
    }

    pub(super) fn parse_init_block(&mut self) -> Result<InitBlockDecl, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::InitBlock, |parser| {
            parser.parse_init_block_inner()
        })
    }

    fn parse_init_block_inner(&mut self) -> Result<InitBlockDecl, Diagnostic> {
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
        self.with_node(scoop_syntax::SyntaxKind::SecondaryConstructor, |parser| {
            parser.parse_secondary_constructor_inner(annotations, visibility)
        })
    }

    fn parse_secondary_constructor_inner(
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
                let doc_start = self.pos;
                let annotations = self.parse_annotations()?;
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
                let documented = Param {
                    annotations,
                    span: Span::new(
                        modifier_span.map_or(name.span.start, |span| span.start),
                        end,
                    ),
                    name,
                    ty,
                    syntax,
                };
                self.attach_documentation(doc_start, documented.span);
                params.push(documented);
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
