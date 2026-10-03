use super::*;

impl Parser {
    pub(super) fn parse_class_body(&mut self) -> Result<(Vec<ClassMember>, u32), Diagnostic> {
        self.bump(); // `{`
        let body_depth = self.brace_depth();
        let mut members = Vec::new();
        let close = loop {
            if matches!(self.peek().kind, TokenKind::RBrace) {
                break self.bump();
            }
            if matches!(self.peek().kind, TokenKind::Eof) {
                return self.unexpected("`}`");
            }
            let start = self.pos;
            let parsed = self.parse_class_member();
            match parsed {
                Ok(member) => {
                    if matches!(member, ClassMember::Companion(_))
                        && members
                            .iter()
                            .any(|existing| matches!(existing, ClassMember::Companion(_)))
                    {
                        self.diagnostics.push(Diagnostic::at(
                            member.span(),
                            "a nominal declaration may contain at most one companion object",
                        ));
                        self.synchronize_body_item(body_depth, start);
                        continue;
                    }
                    members.push(member);
                    if let Err(diagnostic) = self.expect_statement_end() {
                        self.diagnostics.push(diagnostic);
                        self.synchronize_body_item(body_depth, start);
                    }
                }
                Err(diagnostic) => {
                    if self.at_eof() {
                        return Err(diagnostic);
                    }
                    self.diagnostics.push(diagnostic);
                    self.synchronize_body_item(body_depth, start);
                }
            }
        };
        Ok((members, close.span.end))
    }

    fn parse_class_member(&mut self) -> Result<ClassMember, Diagnostic> {
        let prefix = self.parse_member_prefix()?;
        match &self.peek().kind {
            TokenKind::Val | TokenKind::Var => self
                .parse_property(prefix, PropertyContext::ClassOrObject)
                .map(ClassMember::StoredProperty),
            TokenKind::Ident(text) if text == "init" => {
                self.require_empty_member_prefix(&prefix, "an `init` block")?;
                self.parse_init_block().map(ClassMember::InitBlock)
            }
            TokenKind::Ident(text) if text == "constructor" => self
                .require_unmodified_nominal_prefix(&prefix, "constructor")
                .and_then(|()| {
                    self.parse_secondary_constructor(prefix.annotations, prefix.visibility)
                })
                .map(ClassMember::SecondaryConstructor),
            TokenKind::Ident(text) if text == "typealias" => self.unsupported_nested_type_alias(),
            TokenKind::Struct | TokenKind::Enum | TokenKind::Class | TokenKind::Interface => self
                .parse_nested_nominal(prefix)
                .map(Box::new)
                .map(ClassMember::Nested),
            TokenKind::Ident(text) if text == "companion" => {
                self.require_unmodified_nominal_prefix(&prefix, "companion object")?;
                self.parse_companion_object(prefix.annotations, prefix.visibility)
                    .map(Box::new)
                    .map(ClassMember::Companion)
            }
            TokenKind::Ident(text) if text == "object" => {
                self.require_unmodified_nominal_prefix(&prefix, "object")?;
                self.parse_object(prefix.annotations, prefix.visibility)
                    .map(|object| NestedNominalDecl::Object(Box::new(object)))
                    .map(Box::new)
                    .map(ClassMember::Nested)
            }
            TokenKind::Fun => self
                .parse_prefixed_member_function(prefix, FunctionContext::TypeBody)
                .map(ClassMember::Function),
            _ => Err(Diagnostic::at(
                self.peek().span,
                "expected a property, function, constructor, init block, nested declaration, or companion object",
            )),
        }
    }

    pub(in crate::decl) fn parse_prefixed_member_function(
        &mut self,
        prefix: MemberPrefix,
        context: FunctionContext,
    ) -> Result<FunctionDecl, Diagnostic> {
        if prefix.is_const {
            return Err(Diagnostic::at(
                prefix.const_span.expect("const modifier has a span"),
                "`const` is only allowed on `val` properties",
            ));
        }
        if context == FunctionContext::Interface
            && matches!(
                prefix.modifiers.method_modifier,
                Some(MethodModifier::Final | MethodModifier::Open)
            )
        {
            let modifier = match prefix
                .modifiers
                .method_modifier
                .expect("matched explicit interface modality")
            {
                MethodModifier::Final => "final",
                MethodModifier::Open => "open",
                MethodModifier::Abstract => unreachable!(),
            };
            return Err(Diagnostic::at(
                prefix
                    .modifiers
                    .method_modifier_span
                    .expect("explicit modality has a span"),
                format!("`{modifier}` modifier is not allowed on interface methods"),
            ));
        }
        self.parse_function(
            prefix.annotations,
            prefix.visibility,
            prefix.modifiers,
            context,
        )
    }

    fn require_empty_member_prefix(
        &self,
        prefix: &MemberPrefix,
        target: &str,
    ) -> Result<(), Diagnostic> {
        if let Some(annotation) = prefix.annotations.first() {
            return Err(Diagnostic::at(
                annotation.span,
                format!("annotations are not allowed on {target}"),
            ));
        }
        if let VisibilitySyntax::Explicit { span, .. } = prefix.visibility {
            return Err(Diagnostic::at(
                span,
                format!("visibility is not allowed on {target}"),
            ));
        }
        self.require_unmodified_nominal_prefix(prefix, target)
    }

    pub(in crate::decl) fn require_nominal_prefix(
        &self,
        prefix: &MemberPrefix,
        target: &str,
    ) -> Result<(), Diagnostic> {
        if let Some(span) = prefix.modifiers.suspend_span {
            return Err(Diagnostic::at(
                span,
                "`suspend` modifier is only allowed on function declarations",
            ));
        }
        if prefix.is_const
            || prefix.modifiers.is_override
            || prefix.modifiers.operator.is_some()
            || prefix.modifiers.infix.is_some()
        {
            let span = prefix
                .const_span
                .or(prefix.modifiers.suspend_span)
                .or(prefix.modifiers.operator.map(|modifier| modifier.span))
                .or(prefix.modifiers.infix.map(|modifier| modifier.span))
                .or(prefix.modifiers.start.map(|start| Span::new(start, start)))
                .unwrap_or(self.peek().span);
            return Err(Diagnostic::at(
                span,
                format!("invalid modifier on {target} declaration"),
            ));
        }
        Ok(())
    }

    pub(in crate::decl) fn require_unmodified_nominal_prefix(
        &self,
        prefix: &MemberPrefix,
        target: &str,
    ) -> Result<(), Diagnostic> {
        self.require_nominal_prefix(prefix, target)?;
        if let Some(span) = prefix.modifiers.method_modifier_span {
            return Err(Diagnostic::at(
                span,
                format!("class modality is not allowed on {target} declaration"),
            ));
        }
        Ok(())
    }

    pub(in crate::decl) fn parse_nested_nominal(
        &mut self,
        prefix: MemberPrefix,
    ) -> Result<NestedNominalDecl, Diagnostic> {
        self.require_nominal_prefix(&prefix, "nested type")?;
        match self.peek().kind {
            TokenKind::Struct => {
                self.require_unmodified_nominal_prefix(&prefix, "struct")?;
                self.parse_struct(prefix.annotations, prefix.visibility)
                    .map(Box::new)
                    .map(NestedNominalDecl::Struct)
            }
            TokenKind::Enum => {
                self.require_unmodified_nominal_prefix(&prefix, "enum")?;
                self.parse_enum(prefix.annotations, prefix.visibility)
                    .map(Box::new)
                    .map(NestedNominalDecl::Enum)
            }
            TokenKind::Class => {
                let modifier = match prefix.modifiers.method_modifier {
                    Some(MethodModifier::Open) => ClassModifier::Open,
                    Some(MethodModifier::Abstract) => ClassModifier::Abstract,
                    Some(MethodModifier::Final) | None => ClassModifier::Final,
                };
                let modifier_span = prefix.modifiers.method_modifier_span;
                self.parse_class(
                    prefix.annotations,
                    prefix.visibility,
                    modifier,
                    modifier_span,
                )
                .map(Box::new)
                .map(NestedNominalDecl::Class)
            }
            TokenKind::Interface => {
                self.require_unmodified_nominal_prefix(&prefix, "interface")?;
                self.parse_interface(prefix.annotations, prefix.visibility)
                    .map(Box::new)
                    .map(NestedNominalDecl::Interface)
            }
            _ => self.unexpected("nested nominal declaration"),
        }
    }

    pub(super) fn parse_struct_body(&mut self) -> Result<(Vec<StructMember>, u32), Diagnostic> {
        self.bump(); // `{`
        let body_depth = self.brace_depth();
        let mut members = Vec::new();
        let close = loop {
            if matches!(self.peek().kind, TokenKind::RBrace) {
                break self.bump();
            }
            if matches!(self.peek().kind, TokenKind::Eof) {
                return self.unexpected("`}`");
            }
            let start = self.pos;
            let parsed = self.parse_member_prefix().and_then(|prefix| match &self.peek().kind {
                TokenKind::Ident(text) if text == "constructor" => self
                    .require_unmodified_nominal_prefix(&prefix, "constructor")
                    .and_then(|()| {
                        self.parse_secondary_constructor(prefix.annotations, prefix.visibility)
                    })
                    .map(StructMember::SecondaryConstructor),
                TokenKind::Ident(text) if text == "typealias" => {
                    self.unsupported_nested_type_alias()
                }
                TokenKind::Val | TokenKind::Var => self
                    .parse_property(prefix, PropertyContext::ValueType)
                    .map(Box::new)
                    .map(StructMember::Property),
                TokenKind::Ident(text) if text == "init" => {
                    Err(Diagnostic::at(self.peek().span, "`init` blocks are not allowed in structs"))
                }
                TokenKind::Struct | TokenKind::Enum | TokenKind::Class | TokenKind::Interface => {
                    self.parse_nested_nominal(prefix)
                        .map(Box::new)
                        .map(StructMember::Nested)
                }
                TokenKind::Ident(text) if text == "companion" => {
                    self.require_unmodified_nominal_prefix(&prefix, "companion object")?;
                    self.parse_companion_object(prefix.annotations, prefix.visibility)
                        .map(Box::new)
                        .map(StructMember::Companion)
                }
                TokenKind::Ident(text) if text == "object" => {
                    self.require_unmodified_nominal_prefix(&prefix, "object")?;
                    self.parse_object(prefix.annotations, prefix.visibility)
                        .map(|object| NestedNominalDecl::Object(Box::new(object)))
                        .map(Box::new)
                        .map(StructMember::Nested)
                }
                TokenKind::Fun => self
                    .parse_prefixed_member_function(prefix, FunctionContext::TypeBody)
                    .map(Box::new)
                    .map(StructMember::Function),
                _ => Err(Diagnostic::at(
                    self.peek().span,
                    "expected a computed property, function, constructor, nested declaration, or companion object",
                )),
            });
            match parsed {
                Ok(member) => {
                    if matches!(member, StructMember::Companion(_))
                        && members
                            .iter()
                            .any(|existing| matches!(existing, StructMember::Companion(_)))
                    {
                        self.diagnostics.push(Diagnostic::at(
                            member.span(),
                            "a nominal declaration may contain at most one companion object",
                        ));
                        self.synchronize_body_item(body_depth, start);
                        continue;
                    }
                    members.push(member);
                    if let Err(diagnostic) = self.expect_statement_end() {
                        self.diagnostics.push(diagnostic);
                        self.synchronize_body_item(body_depth, start);
                    }
                }
                Err(diagnostic) => {
                    if self.at_eof() {
                        return Err(diagnostic);
                    }
                    self.diagnostics.push(diagnostic);
                    self.synchronize_body_item(body_depth, start);
                }
            }
        };
        Ok((members, close.span.end))
    }
}
