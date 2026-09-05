use super::*;

type ParsedInterfaceBody = (
    Vec<FunctionDecl>,
    Vec<PropertyDecl>,
    Vec<NestedNominalDecl>,
    Option<CompanionObjectDecl>,
    u32,
);

impl Parser {
    /// `struct <name><T, ...>?(val <field>: <type>, ...) (: <interface>, ...)?
    /// ({ <member>, ... })?` — the type parameter list sits between the name
    /// and the constructor `(`, like `enum` (a `<` right after the name is
    /// unambiguous here). The optional body may hold computed properties,
    /// functions, constructors, static nested declarations, and a companion.
    pub(super) fn parse_struct(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: VisibilitySyntax,
    ) -> Result<StructDecl, Diagnostic> {
        let keyword = self.expect("`struct`", |k| matches!(k, TokenKind::Struct))?;
        let name = self.expect_ident("struct name")?;
        let type_params = self.parse_type_params()?;
        let (fields, mut end) = if matches!(self.peek().kind, TokenKind::LParen) {
            self.bump();
            let mut fields = Vec::new();
            let mut vararg_span = None;
            if !matches!(self.peek().kind, TokenKind::RParen) {
                loop {
                    let modifier_span = self.parse_vararg_modifier(&mut vararg_span)?;
                    if matches!(self.peek().kind, TokenKind::Var) {
                        return Err(Diagnostic::at(
                            self.peek().span,
                            "`var` struct fields are not supported (value types are immutable)",
                        ));
                    }
                    let val = self.expect("`val`", |k| matches!(k, TokenKind::Val))?;
                    let field_name = self.expect_ident("field name")?;
                    self.expect("`:`", |k| matches!(k, TokenKind::Colon))?;
                    let ty = self.parse_type_ref()?;
                    let (syntax, end) = self.parse_parameter_syntax(modifier_span, ty.span.end)?;
                    fields.push(FieldDecl {
                        span: Span::new(
                            modifier_span.map_or(val.span.start, |span| span.start),
                            end,
                        ),
                        name: field_name,
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
            let close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
            (StructRepresentationDecl::Declared(fields), close.span.end)
        } else {
            (StructRepresentationDecl::Omitted, name.span.end)
        };
        let supertypes = self.parse_supertypes(&mut end)?;
        let where_clause = self.parse_where_clause()?;
        if let Some(clause) = &where_clause {
            end = clause.span.end;
        }
        let members = if matches!(self.peek().kind, TokenKind::LBrace) {
            let (members, body_end) = self.parse_struct_body()?;
            end = body_end;
            members
        } else {
            Vec::new()
        };
        let start = match visibility {
            VisibilitySyntax::Explicit { span, .. } => Some(span.start),
            VisibilitySyntax::Omitted => None,
        }
        .unwrap_or(keyword.span.start);
        Ok(StructDecl {
            annotations,
            visibility,
            name,
            type_params,
            fields,
            supertypes,
            where_clause,
            members,
            span: Span::new(start, end),
        })
    }

    /// `(open|abstract)? class <name>(<ctor prop>, ...)? (: <supertypes>)?
    /// ({ <fun>, ... })?` (spec 9.1). Constructor properties must be
    /// declared with `val` / `var`. The first supertype may carry
    /// constructor arguments — that makes it the base class
    /// (`Base(args)`); the rest are interfaces. The body holds member
    /// declarations while preserving class initialization order.
    pub(super) fn parse_class(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: VisibilitySyntax,
        modifier: ClassModifier,
        modifier_span: Option<Span>,
    ) -> Result<ClassDecl, Diagnostic> {
        let keyword = self.expect("`class`", |k| matches!(k, TokenKind::Class))?;
        let name = self.expect_ident("class name")?;
        let type_params = self.parse_type_params()?;
        let mut end = name.span.end;
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
                    parameters.push(PrimaryClassParameter {
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
                    });
                    if matches!(self.peek().kind, TokenKind::Comma) {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            end = self
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
            ClassConstructorDecl::Declared(PrimaryConstructorDecl {
                annotations: constructor_annotations,
                visibility: constructor_visibility,
                parameters,
                span: Span::new(constructor_start, end),
            })
        } else {
            ClassConstructorDecl::Omitted
        };
        let supertypes = self.parse_supertypes(&mut end)?;
        let where_clause = self.parse_where_clause()?;
        if let Some(clause) = &where_clause {
            end = clause.span.end;
        }
        let members = if matches!(self.peek().kind, TokenKind::LBrace) {
            let (members, body_end) = self.parse_class_body()?;
            end = body_end;
            members
        } else {
            Vec::new()
        };
        let start = modifier_span
            .map(|span| span.start)
            .into_iter()
            .chain(match visibility {
                VisibilitySyntax::Explicit { span, .. } => Some(span.start),
                VisibilitySyntax::Omitted => None,
            })
            .min()
            .unwrap_or(keyword.span.start);
        Ok(ClassDecl {
            annotations,
            visibility,
            modifier,
            name,
            type_params,
            constructor,
            supertypes,
            where_clause,
            members,
            span: Span::new(start, end),
        })
    }

    pub(super) fn parse_vararg_modifier(
        &mut self,
        existing: &mut Option<Span>,
    ) -> Result<Option<Span>, Diagnostic> {
        if !matches!(self.peek().kind, TokenKind::Vararg) {
            return Ok(None);
        }
        let modifier = self.bump();
        if matches!(self.peek().kind, TokenKind::Vararg) {
            return Err(Diagnostic::at(
                self.peek().span,
                "duplicate `vararg` modifier on parameter",
            ));
        }
        if existing.is_some() {
            return Err(Diagnostic::at(
                modifier.span,
                "a parameter list may declare at most one `vararg` parameter",
            ));
        }
        *existing = Some(modifier.span);
        Ok(Some(modifier.span))
    }

    /// `: Base(args), I1, I2` — a no-op when the next token is not `:`.
    /// Every entry retains whether its argument list was present. HIR, which
    /// knows nominal kinds, decides which entry is the direct base.
    pub(super) fn parse_supertypes(
        &mut self,
        end: &mut u32,
    ) -> Result<Vec<SupertypeSpec>, Diagnostic> {
        let mut supertypes = Vec::new();
        if !matches!(self.peek().kind, TokenKind::Colon) {
            return Ok(supertypes);
        }
        self.bump(); // `:`
        loop {
            let super_name = self.expect_ident("base class or interface name")?;
            let supertype = self.parse_named_type_ref_tail(super_name)?;
            let start = supertype.span.start;
            *end = supertype.span.end;
            let constructor_arguments = if matches!(self.peek().kind, TokenKind::LParen) {
                let (args, args_end) = self.parse_args()?;
                *end = args_end;
                Some(args)
            } else {
                None
            };
            supertypes.push(SupertypeSpec {
                ty: supertype,
                constructor_arguments,
                span: Span::new(start, *end),
            });
            if matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        Ok(supertypes)
    }

    /// `interface <name> { <member>, ... }` — signatures, defaults,
    /// properties, nested declarations, and a companion.
    pub(super) fn parse_interface(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: VisibilitySyntax,
    ) -> Result<InterfaceDecl, Diagnostic> {
        let keyword = self.expect("`interface`", |k| matches!(k, TokenKind::Interface))?;
        let name = self.expect_ident("interface name")?;
        let type_params = self.parse_type_params()?;
        let mut end = name.span.end;
        let supertypes = self.parse_supertypes(&mut end)?;
        let where_clause = self.parse_where_clause()?;
        let (methods, properties, nested, companion, end) = self.parse_interface_body()?;
        let start = match visibility {
            VisibilitySyntax::Explicit { span, .. } => Some(span.start),
            VisibilitySyntax::Omitted => None,
        }
        .unwrap_or(keyword.span.start);
        Ok(InterfaceDecl {
            annotations,
            visibility,
            name,
            type_params,
            supertypes,
            where_clause,
            methods,
            properties,
            nested,
            companion,
            span: Span::new(start, end),
        })
    }

    fn parse_interface_body(&mut self) -> Result<ParsedInterfaceBody, Diagnostic> {
        self.expect("`{`", |kind| matches!(kind, TokenKind::LBrace))?;
        let body_depth = self.brace_depth();
        let mut methods = Vec::new();
        let mut properties = Vec::new();
        let mut nested = Vec::new();
        let mut companion = None;
        let close = loop {
            if matches!(self.peek().kind, TokenKind::RBrace) {
                break self.bump();
            }
            if matches!(self.peek().kind, TokenKind::Eof) {
                return self.unexpected("`}`");
            }
            let start = self.pos;
            let parsed = self.parse_member_prefix().and_then(|prefix| match &self.peek().kind {
                TokenKind::Val | TokenKind::Var => self
                    .parse_property(prefix, PropertyContext::Interface)
                    .map(|property| properties.push(property)),
                TokenKind::Fun => self
                    .parse_prefixed_member_function(prefix, FunctionContext::Interface)
                    .map(|function| methods.push(function)),
                TokenKind::Struct | TokenKind::Enum | TokenKind::Class | TokenKind::Interface => {
                    self.parse_nested_nominal(prefix)
                        .map(|declaration| nested.push(declaration))
                }
                TokenKind::Ident(text) if text == "object" => {
                    self.require_unmodified_nominal_prefix(&prefix, "object")?;
                    self.parse_object(prefix.annotations, prefix.visibility)
                        .map(Box::new)
                        .map(NestedNominalDecl::Object)
                        .map(|declaration| nested.push(declaration))
                }
                TokenKind::Ident(text) if text == "companion" => {
                    self.require_unmodified_nominal_prefix(&prefix, "companion object")?;
                    if companion.is_some() {
                        return Err(Diagnostic::at(
                            self.peek().span,
                            "a nominal declaration may contain at most one companion object",
                        ));
                    }
                    self.parse_companion_object(prefix.annotations, prefix.visibility)
                        .map(|declaration| companion = Some(declaration))
                }
                TokenKind::Ident(text) if text == "init" => Err(Diagnostic::at(
                    self.peek().span,
                    "`init` blocks are not allowed in interfaces",
                )),
                TokenKind::Ident(text) if text == "constructor" => Err(Diagnostic::at(
                    self.peek().span,
                    "constructors are not allowed in interfaces",
                )),
                TokenKind::Ident(text) if text == "typealias" => {
                    self.unsupported_nested_type_alias()
                }
                _ => Err(Diagnostic::at(
                    self.peek().span,
                    "expected an interface function, property, nested declaration, or companion object",
                )),
            });
            match parsed {
                Ok(()) => {
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
        Ok((methods, properties, nested, companion, close.span.end))
    }

    pub(super) fn parse_object(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: VisibilitySyntax,
    ) -> Result<ObjectDecl, Diagnostic> {
        let keyword = self.expect(
            "`object`",
            |kind| matches!(kind, TokenKind::Ident(text) if text == "object"),
        )?;
        let name = self.expect_ident("object name")?;
        if matches!(self.peek().kind, TokenKind::Less | TokenKind::LParen) {
            return Err(Diagnostic::at(
                self.peek().span,
                "object declarations cannot declare type parameters or constructors",
            ));
        }
        let mut end = name.span.end;
        let supertypes = self.parse_supertypes(&mut end)?;
        let members = if matches!(self.peek().kind, TokenKind::LBrace) {
            let (members, body_end) = self.parse_class_body()?;
            end = body_end;
            if let Some(constructor) = members.iter().find_map(|member| match member {
                ClassMember::SecondaryConstructor(constructor) => Some(constructor),
                _ => None,
            }) {
                return Err(Diagnostic::at(
                    constructor.span,
                    "object declarations cannot declare constructors",
                ));
            }
            members
        } else {
            Vec::new()
        };
        let start = match visibility {
            VisibilitySyntax::Explicit { span, .. } => Some(span.start),
            VisibilitySyntax::Omitted => None,
        }
        .unwrap_or(keyword.span.start);
        Ok(ObjectDecl {
            annotations,
            visibility,
            name,
            supertypes,
            members,
            span: Span::new(start, end),
        })
    }

    fn parse_class_body(&mut self) -> Result<(Vec<ClassMember>, u32), Diagnostic> {
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

    pub(super) fn parse_prefixed_member_function(
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

    pub(super) fn require_nominal_prefix(
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

    pub(super) fn require_unmodified_nominal_prefix(
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

    pub(super) fn parse_nested_nominal(
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

    pub(super) fn parse_companion_object(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: VisibilitySyntax,
    ) -> Result<CompanionObjectDecl, Diagnostic> {
        let companion = self.bump();
        self.expect(
            "`object` after `companion`",
            |kind| matches!(kind, TokenKind::Ident(text) if text == "object"),
        )?;
        let name = if matches!(self.peek().kind, TokenKind::Ident(_)) {
            CompanionNameSyntax::Named(self.expect_ident("companion object name")?)
        } else {
            CompanionNameSyntax::Default {
                span: companion.span,
            }
        };
        if matches!(self.peek().kind, TokenKind::Less | TokenKind::LParen) {
            return Err(Diagnostic::at(
                self.peek().span,
                "companion objects cannot declare type parameters or constructors",
            ));
        }
        let mut end = match &name {
            CompanionNameSyntax::Default { span } => span.end,
            CompanionNameSyntax::Named(name) => name.span.end,
        };
        let supertypes = self.parse_supertypes(&mut end)?;
        let members = if matches!(self.peek().kind, TokenKind::LBrace) {
            let (members, body_end) = self.parse_class_body()?;
            end = body_end;
            if let Some(constructor) = members.iter().find_map(|member| match member {
                ClassMember::SecondaryConstructor(constructor) => Some(constructor),
                _ => None,
            }) {
                return Err(Diagnostic::at(
                    constructor.span,
                    "companion objects cannot declare constructors",
                ));
            }
            members
        } else {
            Vec::new()
        };
        let start = match visibility {
            VisibilitySyntax::Explicit { span, .. } => Some(span.start),
            VisibilitySyntax::Omitted => None,
        }
        .unwrap_or(companion.span.start);
        Ok(CompanionObjectDecl {
            annotations,
            visibility,
            name,
            supertypes,
            members,
            span: Span::new(start, end),
        })
    }

    fn parse_struct_body(&mut self) -> Result<(Vec<StructMember>, u32), Diagnostic> {
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

    fn parse_init_block(&mut self) -> Result<InitBlockDecl, Diagnostic> {
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

    fn parse_secondary_constructor(
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

    pub(super) fn parse_member_prefix(&mut self) -> Result<MemberPrefix, Diagnostic> {
        self.parse_declaration_prefix(false)
    }

    pub(super) fn parse_top_level_prefix(&mut self) -> Result<MemberPrefix, Diagnostic> {
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
