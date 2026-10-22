use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PropertyContext {
    TopLevel,
    ClassOrObject,
    Interface,
    ValueType,
}

impl Parser {
    pub(super) fn parse_property(
        &mut self,
        prefix: MemberPrefix,
        context: PropertyContext,
    ) -> Result<PropertyDecl, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::PropertyDecl, |parser| {
            parser.parse_property_inner(prefix, context)
        })
    }

    fn parse_property_inner(
        &mut self,
        prefix: MemberPrefix,
        context: PropertyContext,
    ) -> Result<PropertyDecl, Diagnostic> {
        if let Some(span) = prefix.modifiers.tailrec_span {
            return Err(Diagnostic::at(
                span,
                "`tailrec` is only allowed on functions",
            ));
        }
        if let Some(span) = prefix
            .modifiers
            .suspend_span
            .or(prefix.modifiers.operator.map(|modifier| modifier.span))
            .or(prefix.modifiers.infix.map(|modifier| modifier.span))
        {
            return Err(Diagnostic::at(
                span,
                "properties cannot be `suspend`, `operator`, or `infix`",
            ));
        }
        let modifier = prefix
            .modifiers
            .method_modifier
            .map(|modifier| modifier.kind)
            .unwrap_or_else(|| {
                if context == PropertyContext::Interface
                    || (prefix.modifiers.override_span.is_some()
                        && context != PropertyContext::ValueType)
                {
                    MethodModifier::Open
                } else {
                    MethodModifier::Final
                }
            });
        let annotations = prefix.annotations;
        let visibility = prefix.visibility;
        let is_override = prefix.modifiers.override_span.is_some();
        let is_const = prefix.const_span.is_some();
        let prefix_start = prefix.modifiers.start;
        let keyword = self.expect("`val` or `var`", |kind| {
            matches!(kind, TokenKind::Val | TokenKind::Var)
        })?;
        let mutable = matches!(keyword.kind, TokenKind::Var);
        let type_params = self.parse_type_params()?;

        let receiver_start = self.checkpoint();
        let receiver_annotations = self.parse_annotations()?;
        let mut parsed_name = None;
        let receiver_ty = match self.parse_type_ref() {
            Ok(ty) if matches!(self.peek().kind, TokenKind::Dot | TokenKind::QuestionDot) => {
                Some(self.parse_receiver_separator(ty))
            }
            Ok(ty) => match ty.split_qualified_tail() {
                Ok((receiver, name)) => {
                    parsed_name = Some(name);
                    Some(receiver)
                }
                Err(_) => {
                    self.restore(receiver_start);
                    None
                }
            },
            _ => {
                self.restore(receiver_start);
                None
            }
        }
        .map(|receiver| receiver.with_annotations(receiver_annotations));
        if let Some(receiver) = &receiver_ty
            && context != PropertyContext::TopLevel
        {
            return Err(Diagnostic::at(
                receiver.span,
                "extension properties may only be declared at top level",
            ));
        }
        if receiver_ty.is_none() && !type_params.is_empty() {
            return Err(Diagnostic::at(
                type_params[0].span,
                "only extension properties may declare type parameters",
            ));
        }

        let name = match parsed_name {
            Some(name) => name,
            None => self.expect_ident("property name")?,
        };
        self.record_declaration_name(&name);
        self.record_declaration_prefix(
            visibility,
            scoop_ast::DeclarationHeaderKind::Property {
                is_extension: receiver_ty.is_some(),
            },
        );
        let ty = if matches!(self.peek().kind, TokenKind::Colon) {
            self.bump();
            Some(self.parse_type_ref()?)
        } else {
            None
        };
        let where_clause = self.parse_where_clause()?;
        let signature_end = where_clause.as_ref().map_or_else(
            || ty.as_ref().map_or(name.span.end, |ty| ty.span.end),
            |clause| clause.span.end,
        );

        let (body, end) = if is_const {
            self.expect("`=` in const property", |kind| {
                matches!(kind, TokenKind::Equal)
            })?;
            let expression = self.parse_expr()?;
            let end = expression.span().end;
            (PropertyBodySyntax::Const(Box::new(expression)), end)
        } else if matches!(&self.peek().kind, TokenKind::Ident(text) if text == "by") {
            let by = self.bump();
            let expression = self.parse_expr()?;
            let end = expression.span().end;
            (
                PropertyBodySyntax::Delegated {
                    expression: Box::new(expression),
                    by_span: by.span,
                },
                end,
            )
        } else if matches!(self.peek().kind, TokenKind::Equal) {
            self.bump();
            let expression = self.parse_expr()?;
            let accessors = self.parse_property_accessors()?;
            let end = accessors
                .setter
                .as_ref()
                .map(|setter| setter.span.end)
                .or_else(|| accessors.getter.as_ref().map(|getter| getter.span.end))
                .unwrap_or(expression.span().end);
            (
                PropertyBodySyntax::Initializer {
                    expression: Box::new(expression),
                    accessors,
                },
                end,
            )
        } else {
            let accessors = self.parse_property_accessors()?;
            let accessor_end = accessors
                .setter
                .as_ref()
                .map(|setter| setter.span.end)
                .or_else(|| accessors.getter.as_ref().map(|getter| getter.span.end));
            if modifier == MethodModifier::Abstract && accessor_end.is_some() {
                return Err(Diagnostic::at(
                    accessor_end
                        .map(|end| Span::new(end, end))
                        .expect("checked above"),
                    "an abstract property cannot declare accessor bodies or shorthand",
                ));
            }
            if modifier == MethodModifier::Abstract {
                (
                    PropertyBodySyntax::Abstract,
                    accessor_end.unwrap_or(signature_end),
                )
            } else if accessor_end.is_some() || context == PropertyContext::Interface {
                (
                    PropertyBodySyntax::Computed(accessors),
                    accessor_end.unwrap_or(signature_end),
                )
            } else if annotations
                .iter()
                .any(|annotation| annotation.name.is_ident("Extern"))
            {
                (PropertyBodySyntax::ExternStorage, signature_end)
            } else {
                (PropertyBodySyntax::OptionalOmitted, signature_end)
            }
        };

        if !prefix.modifiers.context_parameters.is_empty()
            && !matches!(
                body,
                PropertyBodySyntax::Computed(_) | PropertyBodySyntax::Abstract
            )
        {
            return Err(Diagnostic::at(
                prefix.modifiers.context_parameters[0].span,
                "contextual properties cannot have backing storage, an initializer or a delegate",
            ));
        }
        let start = prefix_start
            .into_iter()
            .chain(match visibility {
                VisibilitySyntax::Explicit { span, .. } => Some(span.start),
                VisibilitySyntax::Omitted => None,
            })
            .min()
            .unwrap_or(keyword.span.start);
        Ok(PropertyDecl {
            context_parameters: prefix.modifiers.context_parameters,
            annotations,
            visibility,
            modifier,
            is_override,
            mutable,
            receiver_ty,
            type_params,
            where_clause,
            name,
            ty,
            body,
            span: Span::new(start, end),
        })
    }

    fn parse_property_accessors(&mut self) -> Result<AccessorSyntax, Diagnostic> {
        let mut accessors = AccessorSyntax::default();
        loop {
            let checkpoint = self.checkpoint();
            let (annotations, visibility) = self.parse_accessor_prefix()?;
            let token = self.peek().clone();
            let TokenKind::Ident(name) = &token.kind else {
                self.restore(checkpoint);
                break;
            };
            match name.as_str() {
                "get" => {
                    if accessors.getter.is_some() {
                        return Err(Diagnostic::at(token.span, "duplicate property getter"));
                    }
                    if !matches!(visibility, VisibilitySyntax::Omitted) {
                        return Err(Diagnostic::at(
                            token.span,
                            "a getter cannot declare visibility separately from its property",
                        ));
                    }
                    let getter = self.parse_getter(annotations)?;
                    self.attach_documentation(checkpoint.position, getter.span);
                    accessors.getter = Some(getter);
                }
                "set" => {
                    if accessors.setter.is_some() {
                        return Err(Diagnostic::at(token.span, "duplicate property setter"));
                    }
                    let visibility = match visibility {
                        VisibilitySyntax::Explicit { visibility, span } => {
                            SetterVisibilitySyntax::Explicit { visibility, span }
                        }
                        VisibilitySyntax::Omitted => SetterVisibilitySyntax::Inherited,
                    };
                    let setter = self.parse_setter(annotations, visibility)?;
                    self.attach_documentation(checkpoint.position, setter.span);
                    accessors.setter = Some(setter);
                }
                _ => {
                    self.restore(checkpoint);
                    break;
                }
            }
        }
        Ok(accessors)
    }

    fn parse_getter(&mut self, annotations: Vec<Annotation>) -> Result<GetterDecl, Diagnostic> {
        let keyword = self.bump();
        self.expect("`(` after `get`", |kind| matches!(kind, TokenKind::LParen))?;
        let close = self.expect("`)` after `get(`", |kind| matches!(kind, TokenKind::RParen))?;
        let body = self.parse_accessor_body()?;
        let end = accessor_body_end(&body).unwrap_or(close.span.end);
        let start = annotations
            .first()
            .map_or(keyword.span.start, |annotation| annotation.span.start);
        Ok(GetterDecl {
            annotations,
            body,
            span: Span::new(start, end),
        })
    }

    fn parse_setter(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: SetterVisibilitySyntax,
    ) -> Result<SetterDecl, Diagnostic> {
        let keyword = self.bump();
        let open = self.expect("`(` after `set`", |kind| matches!(kind, TokenKind::LParen))?;
        let parameter = if matches!(self.peek().kind, TokenKind::RParen) {
            SetterParameterSyntax::Default {
                span: Span::new(open.span.end, open.span.end),
            }
        } else {
            let doc_start = self.pos;
            self.skip_parameter_compatibility_modifiers();
            let name = self.expect_ident("setter parameter name")?;
            self.attach_documentation(doc_start, name.span);
            SetterParameterSyntax::Named(name)
        };
        let close = self.expect("`)` after setter parameter", |kind| {
            matches!(kind, TokenKind::RParen)
        })?;
        let body = self.parse_accessor_body()?;
        let end = accessor_body_end(&body).unwrap_or(close.span.end);
        let start = annotations
            .first()
            .map(|annotation| annotation.span.start)
            .into_iter()
            .chain(match visibility {
                SetterVisibilitySyntax::Explicit { span, .. } => Some(span.start),
                SetterVisibilitySyntax::Inherited => None,
            })
            .min()
            .unwrap_or(keyword.span.start);
        Ok(SetterDecl {
            annotations,
            visibility,
            parameter,
            body,
            span: Span::new(start, end),
        })
    }

    fn parse_accessor_body(&mut self) -> Result<AccessorBodySyntax, Diagnostic> {
        match self.peek().kind {
            TokenKind::LBrace => self.parse_block().map(AccessorBodySyntax::Block),
            TokenKind::Equal => {
                self.bump();
                self.parse_expr()
                    .map(Box::new)
                    .map(AccessorBodySyntax::Expr)
            }
            _ => Ok(AccessorBodySyntax::Omitted),
        }
    }
}

fn accessor_body_end(body: &AccessorBodySyntax) -> Option<u32> {
    match body {
        AccessorBodySyntax::Block(block) => Some(block.span.end),
        AccessorBodySyntax::Expr(expression) => Some(expression.span().end),
        AccessorBodySyntax::Omitted => None,
    }
}
