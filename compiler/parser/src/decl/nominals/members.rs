use super::*;

impl Parser {
    pub(in crate::decl) fn parse_class_body(
        &mut self,
    ) -> Result<(Vec<ClassMember>, u32), Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::ClassBody, |parser| {
            parser.parse_class_body_inner()
        })
    }

    fn parse_class_body_inner(&mut self) -> Result<(Vec<ClassMember>, u32), Diagnostic> {
        self.bump(); // `{`
        let body_depth = self.brace_depth();
        let mut members = Vec::new();
        let close = loop {
            if matches!(self.peek().kind, TokenKind::RBrace) {
                break self.bump();
            }
            if matches!(self.peek().kind, TokenKind::Eof) {
                return self.missing_token(TokenKind::RBrace, "`}`");
            }
            let start = self.pos;
            let parsed = self.parse_class_member();
            match parsed {
                Ok(member) => {
                    if !matches!(
                        member,
                        ClassMember::InitBlock(_) | ClassMember::ReleaseBlock(_)
                    ) {
                        self.attach_documentation(start, member.span());
                    }
                    if matches!(member, ClassMember::Companion(_))
                        && members
                            .iter()
                            .any(|existing| matches!(existing, ClassMember::Companion(_)))
                    {
                        self.invalidate_nominal_projection();
                        self.diagnostics.push(Diagnostic::at(
                            member.span(),
                            "a nominal declaration may contain at most one companion object",
                        ));
                        self.synchronize_body_item(body_depth, start);
                        continue;
                    }
                    members.push(member);
                    if let Err(diagnostic) = self.expect_statement_end() {
                        self.invalidate_nominal_projection();
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
        self.with_node(scoop_syntax::SyntaxKind::Member, |parser| {
            let checkpoint = parser.checkpoint();
            parser.begin_nominal_member();
            let result = parser.parse_class_member_inner();
            parser.end_class_member(&result, checkpoint);
            result
        })
    }

    fn parse_class_member_inner(&mut self) -> Result<ClassMember, Diagnostic> {
        let prefix = self.parse_member_prefix()?;
        self.nominal_member_kind();
        match &self.peek().kind {
            TokenKind::Val | TokenKind::Var => self
                .parse_property(prefix, PropertyContext::ClassOrObject)
                .map(ClassMember::StoredProperty),
            TokenKind::Ident(text) if text == "init" => {
                self.require_empty_member_prefix(&prefix, "an `init` block")?;
                self.parse_init_block().map(ClassMember::InitBlock)
            }
            TokenKind::Ident(text) if text == "release" => {
                self.require_empty_member_prefix(&prefix, "a `release` block")?;
                self.parse_release_block().map(ClassMember::ReleaseBlock)
            }
            TokenKind::Ident(text) if text == "constructor" => self
                .require_unmodified_nominal_prefix(&prefix, "constructor")
                .and_then(|()| {
                    self.parse_secondary_constructor(prefix.annotations, prefix.visibility)
                })
                .map(ClassMember::SecondaryConstructor),
            TokenKind::Ident(text) if text == "annotation" => self
                .parse_annotation_class(prefix)
                .map(Box::new)
                .map(NestedTypeDecl::AnnotationClass)
                .map(Box::new)
                .map(ClassMember::Nested),
            TokenKind::Ident(text) if text == "typealias" => self
                .parse_type_alias(prefix)
                .map(Box::new)
                .map(NestedTypeDecl::TypeAlias)
                .map(Box::new)
                .map(ClassMember::Nested),
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
                    .map(|object| NestedTypeDecl::Object(Box::new(object)))
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
        if let Some(span) = prefix.const_span {
            return Err(Diagnostic::at(
                span,
                "`const` is only allowed on `val` properties",
            ));
        }
        if context == FunctionContext::Interface
            && let Some(SpannedMethodModifier {
                kind: kind @ (MethodModifier::Final | MethodModifier::Open),
                span,
            }) = prefix.modifiers.method_modifier
        {
            let modifier = if kind == MethodModifier::Final {
                "final"
            } else {
                "open"
            };
            return Err(Diagnostic::at(
                span,
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
        if let Some(span) = prefix
            .const_span
            .or(prefix.modifiers.operator.map(|modifier| modifier.span))
            .or(prefix.modifiers.infix.map(|modifier| modifier.span))
            .or(prefix.modifiers.override_span)
            .or(prefix.modifiers.inline_span)
            .or(prefix.modifiers.tailrec_span)
        {
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
        if let Some(SpannedMethodModifier { span, .. }) = prefix.modifiers.method_modifier {
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
    ) -> Result<NestedTypeDecl, Diagnostic> {
        self.require_nominal_prefix(&prefix, "nested type")?;
        match self.peek().kind {
            TokenKind::Struct => {
                self.require_unmodified_nominal_prefix(&prefix, "struct")?;
                self.parse_struct(prefix.annotations, prefix.visibility)
                    .map(Box::new)
                    .map(NestedTypeDecl::Struct)
            }
            TokenKind::Enum => {
                self.require_unmodified_nominal_prefix(&prefix, "enum")?;
                self.parse_enum(prefix.annotations, prefix.visibility)
                    .map(Box::new)
                    .map(NestedTypeDecl::Enum)
            }
            TokenKind::Class => {
                let modifier = if prefix.modifiers.sealed_span.is_some() {
                    ClassModifier::Sealed
                } else {
                    match prefix
                        .modifiers
                        .method_modifier
                        .map(|modifier| modifier.kind)
                    {
                        Some(MethodModifier::Open) => ClassModifier::Open,
                        Some(MethodModifier::Abstract) => ClassModifier::Abstract,
                        Some(MethodModifier::Final) | None => ClassModifier::Final,
                    }
                };
                let modifier_span = prefix
                    .modifiers
                    .method_modifier
                    .map(|modifier| modifier.span)
                    .or(prefix.modifiers.sealed_span);
                self.parse_class(
                    prefix.annotations,
                    prefix.visibility,
                    modifier,
                    prefix.modifiers.inner_span,
                    modifier_span,
                )
                .map(Box::new)
                .map(NestedTypeDecl::Class)
            }
            TokenKind::Interface => {
                self.require_unmodified_nominal_prefix(&prefix, "interface")?;
                self.parse_interface(
                    prefix.annotations,
                    prefix.visibility,
                    prefix.modifiers.sealed_span,
                )
                .map(Box::new)
                .map(NestedTypeDecl::Interface)
            }
            _ => self.unexpected("nested nominal declaration"),
        }
    }

    pub(super) fn parse_struct_body(&mut self) -> Result<(Vec<StructMember>, u32), Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::StructBody, |parser| {
            parser.parse_struct_body_inner()
        })
    }

    fn parse_struct_body_inner(&mut self) -> Result<(Vec<StructMember>, u32), Diagnostic> {
        self.bump(); // `{`
        let body_depth = self.brace_depth();
        let mut members = Vec::new();
        let close = loop {
            if matches!(self.peek().kind, TokenKind::RBrace) {
                break self.bump();
            }
            if matches!(self.peek().kind, TokenKind::Eof) {
                return self.missing_token(TokenKind::RBrace, "`}`");
            }
            let start = self.pos;
            let checkpoint = self.checkpoint();
            self.begin_nominal_member();
            let parsed = self.parse_member_prefix().and_then(|prefix| {
                self.nominal_member_kind();
                match &self.peek().kind {
                TokenKind::Ident(text) if text == "constructor" => self
                    .require_unmodified_nominal_prefix(&prefix, "constructor")
                    .and_then(|()| {
                        self.parse_secondary_constructor(prefix.annotations, prefix.visibility)
                    })
                    .map(StructMember::SecondaryConstructor),
                TokenKind::Ident(text) if text == "typealias" => {
                    self.parse_type_alias(prefix).map(Box::new).map(NestedTypeDecl::TypeAlias).map(Box::new).map(StructMember::Nested)
                }
                TokenKind::Val | TokenKind::Var => self
                    .parse_property(prefix, PropertyContext::ValueType)
                    .map(Box::new)
                    .map(StructMember::Property),
                TokenKind::Ident(text) if text == "init" => {
                    Err(Diagnostic::at(self.peek().span, "`init` blocks are not allowed in structs"))
                }
                TokenKind::Ident(text) if text == "release" => {
                    self.invalid_release_owner("structs")
                }
                TokenKind::Struct | TokenKind::Enum | TokenKind::Class | TokenKind::Interface => {
                    self.parse_nested_nominal(prefix)
                        .map(Box::new)
                        .map(StructMember::Nested)
                }
                TokenKind::Ident(text) if text == "annotation" => self.parse_annotation_class(prefix).map(Box::new).map(NestedTypeDecl::AnnotationClass).map(Box::new).map(StructMember::Nested),
                TokenKind::Ident(text) if text == "companion" => {
                    self.require_unmodified_nominal_prefix(&prefix, "companion object")?;
                    self.parse_companion_object(prefix.annotations, prefix.visibility)
                        .map(Box::new)
                        .map(StructMember::Companion)
                }
                TokenKind::Ident(text) if text == "object" => {
                    self.require_unmodified_nominal_prefix(&prefix, "object")?;
                    self.parse_object(prefix.annotations, prefix.visibility)
                        .map(|object| NestedTypeDecl::Object(Box::new(object)))
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
            }});
            self.end_struct_member(&parsed, checkpoint);
            match parsed {
                Ok(member) => {
                    self.attach_documentation(start, member.span());
                    if matches!(member, StructMember::Companion(_))
                        && members
                            .iter()
                            .any(|existing| matches!(existing, StructMember::Companion(_)))
                    {
                        self.invalidate_nominal_projection();
                        self.diagnostics.push(Diagnostic::at(
                            member.span(),
                            "a nominal declaration may contain at most one companion object",
                        ));
                        self.synchronize_body_item(body_depth, start);
                        continue;
                    }
                    members.push(member);
                    if let Err(diagnostic) = self.expect_statement_end() {
                        self.invalidate_nominal_projection();
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
