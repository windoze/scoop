use super::*;

impl Parser {
    pub(in crate::decl) fn parse_member_prefix(&mut self) -> Result<MemberPrefix, Diagnostic> {
        self.parse_declaration_prefix(false)
    }

    pub(in crate::decl) fn parse_top_level_prefix(&mut self) -> Result<MemberPrefix, Diagnostic> {
        self.parse_declaration_prefix(true)
    }

    fn parse_declaration_prefix(&mut self, top_level: bool) -> Result<MemberPrefix, Diagnostic> {
        let checkpoint = self.checkpoint();
        let mut annotations = Vec::new();
        let mut visibility = VisibilitySyntax::Omitted;
        let mut modifiers = Modifiers::default();
        let mut const_span = None;
        if matches!(&self.peek().kind, TokenKind::Ident(name) if name == "context") {
            let (parameters, span) = self.parse_context_parameters()?;
            modifiers.context_parameters = parameters;
            modifiers.start = Some(span.start);
        }
        loop {
            let token = self.peek().clone();
            if matches!(&token.kind, TokenKind::Ident(name) if name == "context") {
                return Err(Diagnostic::at(
                    token.span,
                    "a declaration permits one `context` prefix, before annotations and modifiers",
                ));
            }
            if matches!(token.kind, TokenKind::At) {
                annotations.extend(self.parse_annotations()?);
                continue;
            }
            if let TokenKind::Ident(text) = &token.kind
                && matches!(
                    text.as_str(),
                    "public" | "internal" | "private" | "protected"
                )
            {
                if !matches!(visibility, VisibilitySyntax::Omitted) {
                    return Err(Diagnostic::at(
                        token.span,
                        "a declaration may have only one visibility modifier",
                    ));
                }
                visibility = self.parse_visibility()?;
                modifiers.start = modifiers.start.or(Some(token.span.start));
                continue;
            }
            if matches!(token.kind, TokenKind::Suspend) {
                if modifiers.suspend_span.is_some() {
                    return Err(Diagnostic::at(
                        token.span,
                        if top_level {
                            "duplicate `suspend` modifier on top-level function"
                        } else {
                            "duplicate `suspend` modifier on member function"
                        },
                    ));
                }
                let keyword = self.bump();
                modifiers.suspend_span = Some(keyword.span);
                modifiers.start = modifiers.start.or(Some(keyword.span.start));
                continue;
            }
            if matches!(token.kind, TokenKind::Infix) {
                if modifiers.infix.is_some() {
                    return Err(Diagnostic::at(
                        token.span,
                        "duplicate `infix` modifier on member declaration",
                    ));
                }
                let keyword = self.bump();
                modifiers.infix = Some(InfixModifier { span: keyword.span });
                modifiers.start = modifiers.start.or(Some(keyword.span.start));
                continue;
            }
            let TokenKind::Ident(text) = &token.kind else {
                break;
            };
            if matches!(text.as_str(), "inline" | "tailrec") {
                if text == "inline" {
                    modifiers.inline_span = modifiers.inline_span.or(Some(token.span));
                } else {
                    modifiers.tailrec_span = modifiers.tailrec_span.or(Some(token.span));
                }
                self.bump();
                modifiers.start = modifiers.start.or(Some(token.span.start));
                continue;
            }
            if text == "sealed" {
                if modifiers.sealed_span.is_some() {
                    return Err(Diagnostic::at(token.span, "duplicate `sealed` modifier"));
                }
                modifiers.sealed_span = Some(self.bump().span);
                modifiers.start = modifiers.start.or(Some(token.span.start));
                continue;
            }
            if text == "const" {
                if const_span.is_some() {
                    return Err(Diagnostic::at(
                        token.span,
                        "duplicate `const` modifier on property",
                    ));
                }
                self.bump();
                const_span = Some(token.span);
                modifiers.start = modifiers.start.or(Some(token.span.start));
                continue;
            }
            if text == "lateinit" {
                return Err(Diagnostic::at(
                    token.span,
                    "`lateinit` is not supported; use `var p: T? = None` or the Option omitted-initializer shorthand",
                ));
            }
            if text == "inner" {
                if modifiers.inner_span.is_some() {
                    return Err(Diagnostic::at(token.span, "duplicate `inner` modifier"));
                }
                modifiers.inner_span = Some(self.bump().span);
                modifiers.start = modifiers.start.or(Some(token.span.start));
                continue;
            }
            if text == "operator" {
                if modifiers.operator.is_some() {
                    return Err(Diagnostic::at(
                        token.span,
                        "duplicate `operator` modifier on function",
                    ));
                }
                let keyword = self.bump();
                modifiers.operator = Some(OperatorModifier { span: keyword.span });
                modifiers.start = modifiers.start.or(Some(keyword.span.start));
                continue;
            }
            let modality = match text.as_str() {
                "open" => Some(MethodModifier::Open),
                "final" => Some(MethodModifier::Final),
                "abstract" => Some(MethodModifier::Abstract),
                "override" => None,
                _ => break,
            };
            if text == "override" {
                if modifiers.override_span.is_some() {
                    return Err(Diagnostic::at(
                        token.span,
                        "duplicate `override` modifier on member function",
                    ));
                }
                modifiers.override_span = Some(token.span);
            } else if let Some(modality) = modality {
                if let Some(existing) = modifiers.method_modifier {
                    let existing = match existing.kind {
                        MethodModifier::Final => "final",
                        MethodModifier::Open => "open",
                        MethodModifier::Abstract => "abstract",
                    };
                    if existing == text {
                        return Err(Diagnostic::at(
                            token.span,
                            format!("duplicate `{text}` modifier on member function"),
                        ));
                    }
                    return Err(Diagnostic::at(
                        token.span,
                        format!(
                            "`{existing}` and `{text}` cannot be combined on a member function"
                        ),
                    ));
                }
                modifiers.method_modifier = Some(SpannedMethodModifier {
                    kind: modality,
                    span: token.span,
                });
            }
            let keyword = self.bump();
            modifiers.start = modifiers.start.or(Some(keyword.span.start));
        }
        if let Some(span) = modifiers.inner_span
            && !matches!(self.peek().kind, TokenKind::Class)
        {
            return Err(Diagnostic::at(
                span,
                "`inner` is only allowed on class declarations",
            ));
        }
        if let Some(span) = modifiers.sealed_span {
            if !matches!(self.peek().kind, TokenKind::Class | TokenKind::Interface) {
                return Err(Diagnostic::at(
                    span,
                    "`sealed` is only allowed on class or interface declarations",
                ));
            }
            if modifiers.method_modifier.is_some() {
                return Err(Diagnostic::at(
                    span,
                    "`sealed` cannot be combined with open, final or abstract",
                ));
            }
        }
        if !modifiers.context_parameters.is_empty() && const_span.is_some() {
            return Err(Diagnostic::at(
                modifiers.context_parameters[0].span,
                "contextual properties cannot be const",
            ));
        }
        if !modifiers.context_parameters.is_empty()
            && !matches!(
                self.peek().kind,
                TokenKind::Fun | TokenKind::Val | TokenKind::Var
            )
        {
            return Err(Diagnostic::at(
                self.peek().span,
                "`context` may only prefix a named function or computed/abstract property",
            ));
        }
        modifiers.has_syntax_errors = self.has_errors_since(checkpoint);
        Ok(MemberPrefix {
            annotations,
            visibility,
            modifiers,
            const_span,
        })
    }
}
