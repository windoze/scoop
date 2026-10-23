use super::*;

impl Parser {
    pub(in crate::decl) fn parse_object(
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
        let checkpoint = self.checkpoint();
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
        let start = match visibility {
            VisibilitySyntax::Explicit { span, .. } => Some(span.start),
            VisibilitySyntax::Omitted => None,
        }
        .unwrap_or(keyword.span.start);
        let header = scoop_ast::ObjectHeader {
            annotations,
            visibility,
            name,
            supertypes,
            span: Span::new(start, self.tokens[self.pos - 1].span.end),
        };
        self.record_object_header(&header, checkpoint);
        let members = if matches!(self.peek().kind, TokenKind::LBrace) {
            let (members, body_end) = self.parse_class_body()?;
            end = body_end;
            if let Some(constructor) = members.iter().find_map(|member| match member {
                ClassMember::SecondaryConstructor(constructor) => Some(constructor),
                _ => None,
            }) {
                self.invalidate_nominal_projection();
                return Err(Diagnostic::at(
                    constructor.span,
                    "object declarations cannot declare constructors",
                ));
            }
            members
        } else {
            Vec::new()
        };
        Ok(ObjectDecl {
            header,
            members,
            span: Span::new(start, end),
        })
    }

    pub(in crate::decl) fn parse_companion_object(
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
        let checkpoint = self.checkpoint();
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
        let start = match visibility {
            VisibilitySyntax::Explicit { span, .. } => Some(span.start),
            VisibilitySyntax::Omitted => None,
        }
        .unwrap_or(companion.span.start);
        let header = scoop_ast::CompanionObjectHeader {
            annotations,
            visibility,
            name,
            supertypes,
            span: Span::new(start, self.tokens[self.pos - 1].span.end),
        };
        self.record_companion_header(&header, checkpoint);
        let members = if matches!(self.peek().kind, TokenKind::LBrace) {
            let (members, body_end) = self.parse_class_body()?;
            end = body_end;
            if let Some(constructor) = members.iter().find_map(|member| match member {
                ClassMember::SecondaryConstructor(constructor) => Some(constructor),
                _ => None,
            }) {
                self.invalidate_nominal_projection();
                return Err(Diagnostic::at(
                    constructor.span,
                    "companion objects cannot declare constructors",
                ));
            }
            members
        } else {
            Vec::new()
        };
        Ok(CompanionObjectDecl {
            header,
            members,
            span: Span::new(start, end),
        })
    }
}
