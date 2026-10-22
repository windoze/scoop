use super::*;

mod parameters;

impl Parser {
    /// Parse the modifiers accepted syntactically on top-level and local
    /// functions. Target/signature legality of `operator` belongs to HIR.
    pub(crate) fn parse_non_member_function(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: VisibilitySyntax,
        context: FunctionContext,
        has_prefix_errors: bool,
    ) -> Result<FunctionDecl, Diagnostic> {
        let mut modifiers = Modifiers {
            has_syntax_errors: has_prefix_errors,
            ..Modifiers::default()
        };
        loop {
            let token = self.peek().clone();
            match &token.kind {
                TokenKind::Ident(text) if matches!(text.as_str(), "inline" | "tailrec") => {
                    self.bump();
                    modifiers.start = modifiers.start.or(Some(token.span.start));
                }
                TokenKind::Suspend => {
                    if modifiers.suspend_span.is_some() {
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
                TokenKind::Infix => {
                    if modifiers.infix.is_some() {
                        return Err(Diagnostic::at(
                            token.span,
                            "duplicate `infix` modifier on function",
                        ));
                    }
                    self.bump();
                    modifiers.infix = Some(InfixModifier { span: token.span });
                    modifiers.start = modifiers.start.or(Some(token.span.start));
                }
                _ => break,
            }
        }
        if !matches!(self.peek().kind, TokenKind::Fun) {
            if let Some(span) = modifiers.suspend_span
                && modifiers.operator.is_none()
                && modifiers.infix.is_none()
            {
                return Err(Diagnostic::at(
                    span,
                    "`suspend` modifier is only allowed on function declarations",
                ));
            }
            let span = modifiers
                .suspend_span
                .or(modifiers.operator.map(|modifier| modifier.span))
                .or(modifiers.infix.map(|modifier| modifier.span))
                .unwrap_or(self.peek().span);
            return Err(Diagnostic::at(
                span,
                "function modifiers must be followed by `fun`",
            ));
        }
        self.parse_function(annotations, visibility, modifiers, context)
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
        visibility: VisibilitySyntax,
        modifiers: Modifiers,
        context: FunctionContext,
    ) -> Result<FunctionDecl, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::FunctionDecl, |parser| {
            parser.parse_function_inner(annotations, visibility, modifiers, context)
        })
    }

    fn parse_function_inner(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: VisibilitySyntax,
        modifiers: Modifiers,
        context: FunctionContext,
    ) -> Result<FunctionDecl, Diagnostic> {
        let checkpoint = self.checkpoint();
        let fun = self.expect("`fun`", |k| matches!(k, TokenKind::Fun))?;
        let type_params = self.parse_type_params()?;
        let receiver_start = self.checkpoint();
        let receiver_annotations = self.parse_annotations()?;
        let mut parsed_name = None;
        let receiver_ty = match self.parse_type_ref() {
            Ok(ty) if matches!(self.peek().kind, TokenKind::Dot | TokenKind::QuestionDot) => {
                Some(self.parse_receiver_separator(ty))
            }
            Ok(ty) if matches!(self.peek().kind, TokenKind::LParen) => {
                match ty.split_qualified_tail() {
                    Ok((receiver, name)) => {
                        parsed_name = Some(name);
                        Some(receiver)
                    }
                    Err(_) => {
                        self.restore(receiver_start);
                        None
                    }
                }
            }
            _ => {
                self.restore(receiver_start);
                None
            }
        }
        .map(|receiver| receiver.with_annotations(receiver_annotations));
        if let Some(receiver) = &receiver_ty
            && context != FunctionContext::TopLevel
        {
            return Err(Diagnostic::at(
                receiver.span,
                "extension functions may only be declared at top level",
            ));
        }
        let name = match parsed_name {
            Some(name) => name,
            None => self.expect_ident("function name")?,
        };
        self.record_declaration_name(&name);
        let (params, params_end) = self.parse_function_parameters()?;
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
            .unwrap_or(params_end);
        let body_form = match self.peek().kind {
            TokenKind::LBrace => scoop_ast::FunctionBodyForm::Block,
            TokenKind::Equal => scoop_ast::FunctionBodyForm::Expression,
            _ => scoop_ast::FunctionBodyForm::Omitted,
        };
        let start = annotations
            .first()
            .map(|annotation| annotation.span.start)
            .into_iter()
            .chain(modifiers.start)
            .chain(match visibility {
                VisibilitySyntax::Explicit { span, .. } => Some(span.start),
                VisibilitySyntax::Omitted => None,
            })
            .min()
            .unwrap_or(fun.span.start);
        let modifier = modifiers
            .method_modifier
            .map(|modifier| modifier.kind)
            .unwrap_or_else(|| {
                if context == FunctionContext::Interface {
                    if body_form == scoop_ast::FunctionBodyForm::Omitted {
                        MethodModifier::Abstract
                    } else {
                        MethodModifier::Open
                    }
                } else if modifiers.override_span.is_some() {
                    // Kotlin-compatible rule (spec 9.1): overrides stay
                    // open unless explicitly closed with `final`.
                    MethodModifier::Open
                } else {
                    MethodModifier::Final
                }
            });
        let signature = scoop_ast::FunctionSignature {
            context_parameters: modifiers.context_parameters,
            annotations,
            visibility,
            is_suspend: modifiers.suspend_span.is_some(),
            is_override: modifiers.override_span.is_some(),
            operator: modifiers.operator,
            infix: modifiers.infix,
            modifier,
            receiver_ty,
            name,
            type_params,
            params,
            return_ty,
            where_clause,
            span: Span::new(start, signature_end),
        };
        if !modifiers.has_syntax_errors {
            self.record_function_header(&signature, body_form, checkpoint);
        }
        let (body, end) = match self.peek().kind {
            TokenKind::LBrace | TokenKind::Equal
                if signature.modifier == MethodModifier::Abstract =>
            {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "`abstract` functions must not have a body",
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
            _ if signature.modifier == MethodModifier::Abstract
                || context == FunctionContext::Interface =>
            {
                (FunctionBody::None, signature_end)
            }
            _ => (FunctionBody::None, signature_end),
        };
        Ok(FunctionDecl {
            signature,
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
