use super::*;

mod fields;

impl Parser {
    /// `enum <name><T, ...>? (: <interface>, ...)? { <variant>, ...
    /// (<fun>, ...)? }` (spec 4.2). The interface list (spec 4.4.3) sits
    /// after the name / type parameters, before `{`. Variants separate
    /// like statements, with `,` in place of `;`; M6 adds member
    /// functions after the variants.
    pub(super) fn parse_enum(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: VisibilitySyntax,
    ) -> Result<EnumDecl, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::EnumDecl, |parser| {
            parser.parse_enum_inner(annotations, visibility)
        })
    }

    fn parse_enum_inner(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: VisibilitySyntax,
    ) -> Result<EnumDecl, Diagnostic> {
        let keyword = self.expect("`enum`", |k| matches!(k, TokenKind::Enum))?;
        let name = self.expect_ident("enum name")?;
        let type_params = self.parse_type_params()?;
        let mut end = name.span.end;
        let interfaces = interfaces_only(self.parse_supertypes(&mut end)?)?;
        let where_clause = self.parse_where_clause()?;
        self.expect("`{`", |k| matches!(k, TokenKind::LBrace))?;
        let body_depth = self.brace_depth();
        let mut variants = Vec::new();
        let mut methods = Vec::new();
        let mut properties = Vec::new();
        let mut nested = Vec::new();
        let mut companion = None;
        let mut saw_member = false;
        let close = loop {
            if matches!(self.peek().kind, TokenKind::RBrace) {
                break self.bump();
            }
            if matches!(self.peek().kind, TokenKind::Eof) {
                return self.unexpected("`}`");
            }
            let start = self.pos;
            let annotations = self.parse_annotations()?;
            let member_start = saw_member || self.enum_member_starts_here();
            let parsed = if member_start {
                saw_member = true;
                self.parse_member_prefix().and_then(|mut prefix| {
                    prefix.annotations.splice(0..0, annotations);
                    match &self.peek().kind {
                    TokenKind::Fun => self
                        .parse_prefixed_member_function(prefix, FunctionContext::TypeBody)
                        .map(|method| { self.attach_documentation(start, method.span); methods.push(method); }),
                    TokenKind::Val | TokenKind::Var => self
                        .parse_property(prefix, PropertyContext::ValueType)
                        .map(|property| { self.attach_documentation(start, property.span); properties.push(property); }),
                    TokenKind::Struct
                    | TokenKind::Enum
                    | TokenKind::Class
                    | TokenKind::Interface => self
                        .parse_nested_nominal(prefix)
                        .map(|declaration| { self.attach_documentation(start, declaration.span()); nested.push(declaration); }),
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
                        "`init` blocks are not allowed in enums",
                    )),
                    TokenKind::Ident(text) if text == "release" => {
                        self.invalid_release_owner("enums")
                    }
                    TokenKind::Ident(text) if text == "constructor" => Err(Diagnostic::at(
                        self.peek().span,
                        "secondary constructors are not allowed in enums",
                    )),
                    TokenKind::Ident(text) if text == "typealias" => {
                        self.parse_type_alias(prefix).map(Box::new).map(NestedTypeDecl::TypeAlias).map(|declaration| { self.attach_documentation(start, declaration.span()); nested.push(declaration); })
                    }
                    _ => Err(Diagnostic::at(
                        self.peek().span,
                        "expected an enum property, function, nested declaration, or companion object",
                    )),
                }})
                .and_then(|()| self.expect_statement_end())
            } else {
                self.parse_variant(annotations)
                    .map(|variant| {
                        self.attach_documentation(start, variant.span);
                        variants.push(variant);
                    })
                    .and_then(|()| self.expect_variant_end())
            };
            if let Err(diagnostic) = parsed {
                if self.at_eof() {
                    return Err(diagnostic);
                }
                self.diagnostics.push(diagnostic);
                self.synchronize_body_item(body_depth, start);
            }
        };
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
        Ok(EnumDecl {
            annotations,
            visibility,
            name,
            type_params,
            variants,
            interfaces,
            where_clause,
            methods,
            properties,
            nested,
            companion,
            span: Span::new(start, close.span.end),
        })
    }

    fn enum_member_starts_here(&self) -> bool {
        if matches!(&self.peek().kind, TokenKind::Ident(text) if text == "inner")
            && match &self.tokens[self.pos + 1].kind {
                TokenKind::Class | TokenKind::Struct | TokenKind::Enum | TokenKind::Interface => {
                    true
                }
                TokenKind::Ident(text) => matches!(
                    text.as_str(),
                    "inner"
                        | "open"
                        | "abstract"
                        | "final"
                        | "sealed"
                        | "public"
                        | "internal"
                        | "private"
                        | "protected"
                ),
                _ => false,
            }
        {
            return true;
        }
        if self.release_block_starts_here()
            || self.starts_context_declaration()
            || self.starts_modified_local_function()
            || (matches!(&self.peek().kind, TokenKind::Ident(text) if text == "inline")
                && match &self.tokens[self.pos + 1].kind {
                    TokenKind::Val | TokenKind::Var | TokenKind::At => true,
                    TokenKind::Ident(text) => matches!(
                        text.as_str(),
                        "inline"
                            | "public"
                            | "internal"
                            | "private"
                            | "protected"
                            | "open"
                            | "final"
                            | "abstract"
                            | "override"
                            | "const"
                    ),
                    _ => false,
                })
        {
            return true;
        }
        match &self.peek().kind {
            TokenKind::Fun
            | TokenKind::Suspend
            | TokenKind::Infix
            | TokenKind::Val
            | TokenKind::Var
            | TokenKind::Struct
            | TokenKind::Enum
            | TokenKind::Class
            | TokenKind::Interface => true,
            TokenKind::Ident(text) => matches!(
                text.as_str(),
                "public"
                    | "internal"
                    | "private"
                    | "protected"
                    | "override"
                    | "abstract"
                    | "open"
                    | "final"
                    | "operator"
                    | "const"
                    | "lateinit"
                    | "object"
                    | "companion"
                    | "init"
                    | "constructor"
                    | "typealias"
                    | "annotation"
            ),
            _ => false,
        }
    }

    fn expect_variant_end(&mut self) -> Result<(), Diagnostic> {
        if matches!(self.peek().kind, TokenKind::Comma) {
            while matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
            }
            return Ok(());
        }
        let token = self.peek();
        if token.newline_before || matches!(token.kind, TokenKind::RBrace | TokenKind::Eof) {
            Ok(())
        } else {
            self.unexpected("`,` or newline after variant")
        }
    }

    /// One variant: `Name` (unit), `Name(T, ...)` (positional),
    /// `Name { f: T, ... }` (block-style named fields), or
    /// `Name(val f: T = ..., ...)` (constructor-style named fields with
    /// optional constant defaults).
    fn parse_variant(&mut self, annotations: Vec<Annotation>) -> Result<VariantDecl, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::Variant, |parser| {
            parser.parse_variant_inner(annotations)
        })
    }

    fn parse_variant_inner(
        &mut self,
        annotations: Vec<Annotation>,
    ) -> Result<VariantDecl, Diagnostic> {
        let name = self.expect_ident("variant name")?;
        let start = name.span.start;
        let (kind, end) = match self.peek().kind {
            TokenKind::Equal => {
                self.bump();
                let value = self.parse_expr()?;
                let end = value.span().end;
                (VariantDeclKind::Constant(Box::new(value)), end)
            }
            TokenKind::LParen => self.parse_parenthesized_variant_fields()?,
            TokenKind::LBrace => {
                let (fields, end) = self.parse_named_fields()?;
                (VariantDeclKind::Named(fields), end)
            }
            _ => (VariantDeclKind::Unit, name.span.end),
        };
        Ok(VariantDecl {
            annotations,
            name,
            kind,
            span: Span::new(start, end),
        })
    }
}
