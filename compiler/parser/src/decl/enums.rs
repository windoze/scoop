use super::*;

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
                        .map(|method| methods.push(method)),
                    TokenKind::Val | TokenKind::Var => self
                        .parse_property(prefix, PropertyContext::ValueType)
                        .map(|property| properties.push(property)),
                    TokenKind::Struct
                    | TokenKind::Enum
                    | TokenKind::Class
                    | TokenKind::Interface => self
                        .parse_nested_nominal(prefix)
                        .map(|declaration| nested.push(declaration)),
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
                        self.unsupported_nested_type_alias()
                    }
                    _ => Err(Diagnostic::at(
                        self.peek().span,
                        "expected an enum property, function, nested declaration, or companion object",
                    )),
                }})
                .and_then(|()| self.expect_statement_end())
            } else {
                self.parse_variant(annotations)
                    .map(|variant| variants.push(variant))
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
        if self.release_block_starts_here() || self.starts_context_declaration() {
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
        let name = self.expect_ident("variant name")?;
        let start = name.span.start;
        let (kind, end) = match self.peek().kind {
            TokenKind::LParen => {
                self.bump();
                let mut annotations = self.parse_annotations()?;
                if matches!(
                    self.peek().kind,
                    TokenKind::Val | TokenKind::Var | TokenKind::Vararg
                ) {
                    let (fields, end) = self.parse_constructor_fields(annotations)?;
                    (VariantDeclKind::Constructor(fields), end)
                } else {
                    let mut fields = Vec::new();
                    loop {
                        let ty = self.parse_type_ref()?;
                        let span = ty.span;
                        fields.push(scoop_ast::PositionalVariantFieldDecl {
                            annotations,
                            ty,
                            span,
                        });
                        if !matches!(self.peek().kind, TokenKind::Comma) {
                            break;
                        }
                        self.bump();
                        annotations = self.parse_annotations()?;
                    }
                    let close = self.expect("`)`", |kind| matches!(kind, TokenKind::RParen))?;
                    (VariantDeclKind::Positional(fields), close.span.end)
                }
            }
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

    /// `(val f: T = default, ...)` — the `(` is the current token and the
    /// caller has checked that a `val`/`var` follows it.
    fn parse_constructor_fields(
        &mut self,
        mut annotations: Vec<Annotation>,
    ) -> Result<(Vec<VariantFieldDecl>, u32), Diagnostic> {
        let mut fields = Vec::new();
        let mut vararg_span = None;
        loop {
            let modifier_span = self.parse_vararg_modifier(&mut vararg_span)?;
            if matches!(self.peek().kind, TokenKind::Var) {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "`var` variant fields are not supported (value types are immutable)",
                ));
            }
            let val = self.expect("`val`", |k| matches!(k, TokenKind::Val))?;
            let field_name = self.expect_ident("field name")?;
            self.expect("`:`", |k| matches!(k, TokenKind::Colon))?;
            let ty = self.parse_type_ref()?;
            let (syntax, end) = self.parse_parameter_syntax(modifier_span, ty.span.end)?;
            fields.push(VariantFieldDecl {
                annotations,
                span: Span::new(modifier_span.map_or(val.span.start, |span| span.start), end),
                name: field_name,
                ty,
                syntax,
            });
            if matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
                annotations = self.parse_annotations()?;
            } else {
                break;
            }
        }
        let close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
        Ok((fields, close.span.end))
    }

    /// `{ f1: T1, f2: T2 }` — the `{` is the current token. Fields
    /// separate like variants: commas or newlines. Defaults require the
    /// constructor-style form.
    fn parse_named_fields(&mut self) -> Result<(Vec<VariantFieldDecl>, u32), Diagnostic> {
        self.bump(); // `{`
        if matches!(self.peek().kind, TokenKind::RBrace) {
            // A fieldless named variant is outside the M4 subset.
            return self.unexpected("field declaration");
        }
        let mut fields = Vec::new();
        loop {
            let annotations = self.parse_annotations()?;
            let field_name = self.expect_ident("field name")?;
            self.expect("`:`", |k| matches!(k, TokenKind::Colon))?;
            let ty = self.parse_type_ref()?;
            if matches!(self.peek().kind, TokenKind::Equal) {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "variant field defaults require the constructor-style form (milestone M4)",
                ));
            }
            fields.push(VariantFieldDecl {
                annotations,
                span: Span::new(field_name.span.start, ty.span.end),
                name: field_name,
                ty,
                syntax: ParameterSyntax::Required,
            });
            if matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
            } else {
                let token = self.peek();
                if !token.newline_before && !matches!(token.kind, TokenKind::RBrace) {
                    return self.unexpected("`,` or newline after field");
                }
            }
            if matches!(self.peek().kind, TokenKind::RBrace) {
                break;
            }
            if matches!(self.peek().kind, TokenKind::Eof) {
                return self.unexpected("`}`");
            }
        }
        let close = self.expect("`}`", |k| matches!(k, TokenKind::RBrace))?;
        Ok((fields, close.span.end))
    }
}
