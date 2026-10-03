use super::*;

impl Parser {
    pub(in crate::decl) fn parse_member_prefix(&mut self) -> Result<MemberPrefix, Diagnostic> {
        self.parse_declaration_prefix(false)
    }

    pub(in crate::decl) fn parse_top_level_prefix(&mut self) -> Result<MemberPrefix, Diagnostic> {
        self.parse_declaration_prefix(true)
    }

    fn parse_declaration_prefix(&mut self, top_level: bool) -> Result<MemberPrefix, Diagnostic> {
        let mut annotations = Vec::new();
        let mut visibility = VisibilitySyntax::Omitted;
        let mut modifiers = Modifiers::default();
        let mut is_const = false;
        let mut const_span = None;
        loop {
            let token = self.peek().clone();
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
                if modifiers.is_suspend {
                    return Err(Diagnostic::at(
                        token.span,
                        if top_level {
                            "duplicate `suspend` modifier on top-level function"
                        } else {
                            "duplicate `suspend` modifier on member function"
                        },
                    ));
                }
                modifiers.is_suspend = true;
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
            if text == "const" {
                if is_const {
                    return Err(Diagnostic::at(
                        token.span,
                        "duplicate `const` modifier on property",
                    ));
                }
                self.bump();
                is_const = true;
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
                return Err(Diagnostic::at(
                    token.span,
                    "`inner` declarations are not supported; nested declarations are static",
                ));
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
                if modifiers.is_override {
                    return Err(Diagnostic::at(
                        token.span,
                        "duplicate `override` modifier on member function",
                    ));
                }
                modifiers.is_override = true;
                modifiers.override_span = Some(token.span);
            } else if let Some(modality) = modality {
                if let Some(existing) = modifiers.method_modifier {
                    let existing = match existing {
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
                modifiers.method_modifier = Some(modality);
                modifiers.method_modifier_span = Some(token.span);
            }
            let keyword = self.bump();
            modifiers.start = modifiers.start.or(Some(keyword.span.start));
        }
        Ok(MemberPrefix {
            annotations,
            visibility,
            modifiers,
            is_const,
            const_span,
        })
    }
}
