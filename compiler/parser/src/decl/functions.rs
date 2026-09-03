use super::*;

impl Parser {
    /// Parse the modifiers accepted syntactically on top-level and local
    /// functions. Target/signature legality of `operator` belongs to HIR.
    pub(crate) fn parse_non_member_function(
        &mut self,
        annotations: Vec<Annotation>,
        context: FunctionContext,
    ) -> Result<FunctionDecl, Diagnostic> {
        let mut modifiers = Modifiers::default();
        loop {
            let token = self.peek().clone();
            match &token.kind {
                TokenKind::Suspend => {
                    if modifiers.is_suspend {
                        let location = if context == FunctionContext::TopLevel {
                            "top-level "
                        } else {
                            ""
                        };
                        return Err(Diagnostic::at(
                            token.span,
                            format!("duplicate `suspend` modifier on {location}function"),
                        ));
                    }
                    self.bump();
                    modifiers.is_suspend = true;
                    modifiers.suspend_span = Some(token.span);
                    modifiers.start = modifiers.start.or(Some(token.span.start));
                }
                TokenKind::Ident(text) if text == "operator" => {
                    if modifiers.operator.is_some() {
                        return Err(Diagnostic::at(
                            token.span,
                            "duplicate `operator` modifier on function",
                        ));
                    }
                    self.bump();
                    modifiers.operator = Some(OperatorModifier { span: token.span });
                    modifiers.start = modifiers.start.or(Some(token.span.start));
                }
                _ => break,
            }
        }
        if !matches!(self.peek().kind, TokenKind::Fun) {
            if modifiers.is_suspend && modifiers.operator.is_none() {
                return Err(Diagnostic::at(
                    modifiers
                        .suspend_span
                        .expect("suspend modifier has a source span"),
                    "`suspend` modifier is only allowed on function declarations",
                ));
            }
            let span = modifiers
                .suspend_span
                .or(modifiers.operator.map(|modifier| modifier.span))
                .unwrap_or(self.peek().span);
            return Err(Diagnostic::at(
                span,
                "function modifiers must be followed by `fun`",
            ));
        }
        self.parse_function(annotations, modifiers, context)
    }

    /// `(open|final|abstract|override)* fun <T, ...>? (<receiver>.)?<name>(<param>, ...)?: <ret>? <body>?`
    /// — the type parameter list sits between `fun` and the name (a `<`
    /// right after `fun` is unambiguous here), parameters carry mandatory
    /// type annotations, and the return type defaults to `Unit` when
    /// absent. The body is a block or an expression body (`= expr`);
    /// bodyless declarations (`FunctionBody::None`) are `abstract fun`,
    /// interface method signatures, and `@Intrinsic` functions (spec 13.1).
    /// A non-abstract, non-intrinsic function outside an interface must
    /// have a body.
    pub(crate) fn parse_function(
        &mut self,
        annotations: Vec<Annotation>,
        modifiers: Modifiers,
        context: FunctionContext,
    ) -> Result<FunctionDecl, Diagnostic> {
        let fun = self.expect("`fun`", |k| matches!(k, TokenKind::Fun))?;
        let type_params = self.parse_type_params()?;
        let receiver_start = self.pos;
        let receiver_ty = match self.parse_type_ref() {
            Ok(ty) if matches!(self.peek().kind, TokenKind::Dot) => {
                self.bump();
                Some(ty)
            }
            _ => {
                self.pos = receiver_start;
                None
            }
        };
        if let Some(receiver) = &receiver_ty
            && context != FunctionContext::TopLevel
        {
            return Err(Diagnostic::at(
                receiver.span,
                "extension functions may only be declared at top level",
            ));
        }
        let name = self.expect_ident("function name")?;
        self.expect("`(`", |k| matches!(k, TokenKind::LParen))?;
        let mut params = Vec::new();
        let mut vararg_span = None;
        if !matches!(self.peek().kind, TokenKind::RParen) {
            loop {
                let modifier_span = if matches!(self.peek().kind, TokenKind::Vararg) {
                    let modifier = self.bump();
                    if matches!(self.peek().kind, TokenKind::Vararg) {
                        return Err(Diagnostic::at(
                            self.peek().span,
                            "duplicate `vararg` modifier on parameter",
                        ));
                    }
                    if vararg_span.is_some() {
                        return Err(Diagnostic::at(
                            modifier.span,
                            "a parameter list may declare at most one `vararg` parameter",
                        ));
                    }
                    vararg_span = Some(modifier.span);
                    Some(modifier.span)
                } else {
                    None
                };
                let param_name = self.expect_ident("parameter name")?;
                self.expect("`:`", |k| matches!(k, TokenKind::Colon))?;
                let ty = self.parse_type_ref()?;
                let (syntax, end) = self.parse_parameter_syntax(modifier_span, ty.span.end)?;
                params.push(Param {
                    span: Span::new(
                        modifier_span.map_or(param_name.span.start, |span| span.start),
                        end,
                    ),
                    name: param_name,
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
        let params_close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
        let return_ty = if matches!(self.peek().kind, TokenKind::Colon) {
            self.bump();
            Some(self.parse_type_ref()?)
        } else {
            None
        };
        let where_clause = self.parse_where_clause()?;
        let signature_end = where_clause
            .as_ref()
            .map(|clause| clause.span.end)
            .or_else(|| return_ty.as_ref().map(|ty| ty.span.end))
            .unwrap_or(params_close.span.end);
        let (body, end) = match self.peek().kind {
            TokenKind::LBrace | TokenKind::Equal
                if modifiers.method_modifier == Some(MethodModifier::Abstract) =>
            {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "`abstract` functions must not have a body",
                ));
            }
            TokenKind::LBrace | TokenKind::Equal if context == FunctionContext::Interface => {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "interface method bodies are not supported yet (milestone M6)",
                ));
            }
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                let end = block.span.end;
                (FunctionBody::Block(block), end)
            }
            TokenKind::Equal => {
                self.bump();
                let expr = self.parse_expr()?;
                let end = expr.span().end;
                (FunctionBody::Expr(Box::new(expr)), end)
            }
            _ if modifiers.method_modifier == Some(MethodModifier::Abstract)
                || context == FunctionContext::Interface =>
            {
                (FunctionBody::None, signature_end)
            }
            _ => (FunctionBody::None, signature_end),
        };
        let start = annotations
            .first()
            .map(|annotation| annotation.span.start)
            .or(modifiers.start)
            .unwrap_or(fun.span.start);
        let modifier = modifiers.method_modifier.unwrap_or_else(|| {
            if context == FunctionContext::Interface {
                MethodModifier::Abstract
            } else if modifiers.is_override {
                // Kotlin-compatible rule (spec 9.1): overrides stay
                // open unless explicitly closed with `final`.
                MethodModifier::Open
            } else {
                MethodModifier::Final
            }
        });
        Ok(FunctionDecl {
            annotations,
            is_suspend: modifiers.is_suspend,
            is_override: modifiers.is_override,
            operator: modifiers.operator,
            modifier,
            receiver_ty,
            name,
            type_params,
            params,
            return_ty,
            where_clause,
            body,
            span: Span::new(start, end),
        })
    }

    /// Parses the optional default following a parameter type and combines
    /// it with an already-consumed `vararg` modifier.
    pub(super) fn parse_parameter_syntax(
        &mut self,
        modifier_span: Option<Span>,
        type_end: u32,
    ) -> Result<(ParameterSyntax, u32), Diagnostic> {
        let explicit_default = if matches!(self.peek().kind, TokenKind::Equal) {
            let equals = self.bump();
            let expression = self.parse_expr()?;
            let end = expression.span().end;
            Some((expression, equals.span, end))
        } else {
            None
        };
        Ok(match (modifier_span, explicit_default) {
            (None, None) => (ParameterSyntax::Required, type_end),
            (None, Some((expression, equals_span, end))) => (
                ParameterSyntax::Default {
                    expression,
                    equals_span,
                },
                end,
            ),
            (Some(modifier_span), None) => (
                ParameterSyntax::Vararg {
                    modifier_span,
                    default: VarargDefaultSyntax::EmptyWhenOmitted,
                },
                type_end,
            ),
            (Some(modifier_span), Some((expression, equals_span, end))) => (
                ParameterSyntax::Vararg {
                    modifier_span,
                    default: VarargDefaultSyntax::Expression {
                        expression,
                        equals_span,
                    },
                },
                end,
            ),
        })
    }
}
