use super::*;

mod constructors;
mod fields;
mod interfaces;
mod members;
mod prefix;
mod release;
mod supertypes;

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
        self.with_node(scoop_syntax::SyntaxKind::StructDecl, |parser| {
            parser.record_declaration_prefix(visibility, scoop_ast::DeclarationHeaderKind::Type);
            let checkpoint = parser.checkpoint();
            let declaration = parser.parse_struct_inner(annotations, visibility)?;
            parser.record_declaration_source_span(declaration.span, checkpoint);
            Ok(declaration)
        })
    }

    fn parse_struct_inner(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: VisibilitySyntax,
    ) -> Result<StructDecl, Diagnostic> {
        let checkpoint = self.checkpoint();
        let keyword = self.expect("`struct`", |k| matches!(k, TokenKind::Struct))?;
        let name = self.expect_ident("struct name")?;
        self.record_declaration_name(&name);
        let type_params = self.parse_type_params()?;
        let (fields, mut end) = if matches!(self.peek().kind, TokenKind::LParen) {
            let (fields, end) = self.parse_struct_fields()?;
            (StructRepresentationDecl::Declared(fields), end)
        } else {
            (StructRepresentationDecl::Omitted, name.span.end)
        };
        let supertypes = self.parse_supertypes(&mut end)?;
        let where_clause = self.parse_where_clause()?;
        if let Some(clause) = &where_clause {
            end = clause.span.end;
        }
        let start = match visibility {
            VisibilitySyntax::Explicit { span, .. } => Some(span.start),
            VisibilitySyntax::Omitted => None,
        }
        .unwrap_or(keyword.span.start);
        let header = scoop_ast::StructHeader {
            annotations,
            visibility,
            name,
            type_params,
            fields,
            supertypes,
            where_clause,
            span: Span::new(start, self.tokens[self.pos - 1].span.end),
        };
        self.record_struct_header(&header, checkpoint);
        let members = if matches!(self.peek().kind, TokenKind::LBrace) {
            let (members, body_end) = self.parse_struct_body()?;
            end = body_end;
            members
        } else {
            Vec::new()
        };
        Ok(StructDecl {
            header,
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
        self.with_node(scoop_syntax::SyntaxKind::ClassDecl, |parser| {
            parser.record_declaration_prefix(visibility, scoop_ast::DeclarationHeaderKind::Type);
            let checkpoint = parser.checkpoint();
            let declaration = parser.parse_class_inner(
                annotations,
                visibility,
                modifier,
                inner_span,
                modifier_span,
            )?;
            parser.record_declaration_source_span(declaration.span, checkpoint);
            Ok(declaration)
        })
    }

    fn parse_class_inner(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: VisibilitySyntax,
        modifier: ClassModifier,
        inner_span: Option<Span>,
        modifier_span: Option<Span>,
    ) -> Result<ClassDecl, Diagnostic> {
        let checkpoint = self.checkpoint();
        let keyword = self.expect("`class`", |k| matches!(k, TokenKind::Class))?;
        let name = self.expect_ident("class name")?;
        self.record_declaration_name(&name);
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
        let header = scoop_ast::ClassHeader {
            annotations,
            visibility,
            modifier,
            inner_span,
            name,
            type_params,
            constructor,
            supertypes,
            where_clause,
            span: Span::new(start, self.tokens[self.pos - 1].span.end),
        };
        self.record_class_header(&header, checkpoint);
        let members = if matches!(self.peek().kind, TokenKind::LBrace) {
            let (members, body_end) = self.parse_class_body()?;
            end = body_end;
            members
        } else {
            Vec::new()
        };
        Ok(ClassDecl {
            header,
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
        self.with_node(scoop_syntax::SyntaxKind::InterfaceDecl, |parser| {
            parser.record_declaration_prefix(visibility, scoop_ast::DeclarationHeaderKind::Type);
            let checkpoint = parser.checkpoint();
            let declaration = parser.parse_interface_inner(annotations, visibility, sealed_span)?;
            parser.record_declaration_source_span(declaration.span, checkpoint);
            Ok(declaration)
        })
    }

    fn parse_interface_inner(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: VisibilitySyntax,
        sealed_span: Option<Span>,
    ) -> Result<InterfaceDecl, Diagnostic> {
        let checkpoint = self.checkpoint();
        let keyword = self.expect("`interface`", |k| matches!(k, TokenKind::Interface))?;
        let name = self.expect_ident("interface name")?;
        self.record_declaration_name(&name);
        let type_params = self.parse_type_params()?;
        let mut end = name.span.end;
        let supertypes = self.parse_supertypes(&mut end)?;
        let where_clause = self.parse_where_clause()?;
        let start = match visibility {
            VisibilitySyntax::Explicit { span, .. } => Some(span.start),
            VisibilitySyntax::Omitted => None,
        }
        .into_iter()
        .chain(sealed_span.map(|span| span.start))
        .min()
        .unwrap_or(keyword.span.start);
        let header = scoop_ast::InterfaceHeader {
            sealed_span,
            annotations,
            visibility,
            name,
            type_params,
            supertypes,
            where_clause,
            span: Span::new(start, self.tokens[self.pos - 1].span.end),
        };
        if matches!(self.peek().kind, TokenKind::LBrace) {
            self.record_interface_header(&header, checkpoint);
        }
        let (methods, properties, nested, companion, end) = self.parse_interface_body()?;
        Ok(InterfaceDecl {
            header,
            methods,
            properties,
            nested,
            companion,
            span: Span::new(start, end),
        })
    }

    pub(super) fn parse_object(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: VisibilitySyntax,
    ) -> Result<ObjectDecl, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::ObjectDecl, |parser| {
            parser
                .record_declaration_prefix(visibility, scoop_ast::DeclarationHeaderKind::Singleton);
            let checkpoint = parser.checkpoint();
            let declaration = parser.parse_object_inner(annotations, visibility)?;
            parser.record_declaration_source_span(declaration.span, checkpoint);
            Ok(declaration)
        })
    }

    fn parse_object_inner(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: VisibilitySyntax,
    ) -> Result<ObjectDecl, Diagnostic> {
        let keyword = self.expect(
            "`object`",
            |kind| matches!(kind, TokenKind::Ident(text) if text == "object"),
        )?;
        let name = self.expect_ident("object name")?;
        self.record_declaration_name(&name);
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
        self.with_node(scoop_syntax::SyntaxKind::CompanionDecl, |parser| {
            parser
                .record_declaration_prefix(visibility, scoop_ast::DeclarationHeaderKind::Singleton);
            let checkpoint = parser.checkpoint();
            let declaration = parser.parse_companion_object_inner(annotations, visibility)?;
            parser.record_declaration_source_span(declaration.span, checkpoint);
            Ok(declaration)
        })
    }

    fn parse_companion_object_inner(
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
        match &name {
            CompanionNameSyntax::Named(name) => self.record_declaration_name(name),
            CompanionNameSyntax::Default { span } => self.record_syntax_name("Companion", *span),
        }
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
