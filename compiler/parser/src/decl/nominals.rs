use super::*;

mod constructors;
mod members;
mod prefix;
mod release;

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
                    let annotations = self.parse_annotations()?;
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
                        annotations,
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
                    parameters.push(PrimaryClassParameter {
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
                TokenKind::Ident(text) if text == "annotation" => self.parse_annotation_class(prefix).map(Box::new).map(NestedNominalDecl::AnnotationClass).map(|declaration| nested.push(declaration)),
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
                TokenKind::Ident(text) if text == "release" => {
                    self.invalid_release_owner("interfaces")
                }
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
}
