use super::*;

mod constructors;
mod interface_implementations;
mod members;
mod prefix;
mod release;
mod supertypes;

type ParsedInterfaceBody = (
    Vec<FunctionDecl>,
    Vec<PropertyDecl>,
    Vec<NestedTypeDecl>,
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
                    let doc_start = self.pos;
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
                    let documented = FieldDecl {
                        annotations,
                        span: Span::new(
                            modifier_span.map_or(val.span.start, |span| span.start),
                            end,
                        ),
                        name: field_name,
                        ty,
                        syntax,
                    };
                    self.attach_documentation(doc_start, documented.span);
                    fields.push(documented);
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
        inner_span: Option<Span>,
        modifier_span: Option<Span>,
    ) -> Result<ClassDecl, Diagnostic> {
        let keyword = self.expect("`class`", |k| matches!(k, TokenKind::Class))?;
        let name = self.expect_ident("class name")?;
        let type_params = self.parse_type_params()?;
        let mut end = name.span.end;
        let constructor = self.parse_primary_constructor()?;
        if let ClassConstructorDecl::Declared(primary) = &constructor {
            end = primary.span.end;
        }
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
            .chain(inner_span.map(|span| span.start))
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
            inner_span,
            name,
            type_params,
            constructor,
            supertypes,
            where_clause,
            members,
            span: Span::new(start, end),
        })
    }

    /// `interface <name> { <member>, ... }` — signatures, defaults,
    /// properties, nested declarations, and a companion.
    pub(super) fn parse_interface(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: VisibilitySyntax,
        sealed_span: Option<Span>,
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
        .into_iter()
        .chain(sealed_span.map(|span| span.start))
        .min()
        .unwrap_or(keyword.span.start);
        Ok(InterfaceDecl {
            sealed_span,
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
                    .map(|property| { self.attach_documentation(start, property.span); properties.push(property); }),
                TokenKind::Fun => self
                    .parse_prefixed_member_function(prefix, FunctionContext::Interface)
                    .map(|function| { self.attach_documentation(start, function.span); methods.push(function); }),
                TokenKind::Struct | TokenKind::Enum | TokenKind::Class | TokenKind::Interface => {
                    self.parse_nested_nominal(prefix)
                        .map(|declaration| { self.attach_documentation(start, declaration.span()); nested.push(declaration); })
                }
                TokenKind::Ident(text) if text == "annotation" => self.parse_annotation_class(prefix).map(Box::new).map(NestedTypeDecl::AnnotationClass).map(|declaration| { self.attach_documentation(start, declaration.span()); nested.push(declaration); }),
                TokenKind::Ident(text) if text == "object" => {
                    self.require_unmodified_nominal_prefix(&prefix, "object")?;
                    self.parse_object(prefix.annotations, prefix.visibility)
                        .map(Box::new)
                        .map(NestedTypeDecl::Object)
                        .map(|declaration| { self.attach_documentation(start, declaration.span()); nested.push(declaration); })
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
                        .map(|declaration| { self.attach_documentation(start, declaration.span); companion = Some(declaration); })
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
                    self.parse_type_alias(prefix).map(Box::new).map(NestedTypeDecl::TypeAlias).map(|declaration| { self.attach_documentation(start, declaration.span()); nested.push(declaration); })
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
        let name = if matches!(
            self.peek().kind,
            TokenKind::Ident(_) | TokenKind::EscapedIdent(_)
        ) {
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
