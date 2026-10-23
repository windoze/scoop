use super::*;

mod fields;
mod members;

use crate::parser::NominalMember;

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
            parser.record_declaration_prefix(visibility, scoop_ast::DeclarationHeaderKind::Type);
            let checkpoint = parser.checkpoint();
            let declaration = parser.parse_enum_inner(annotations, visibility)?;
            parser.record_declaration_source_span(declaration.span, checkpoint);
            Ok(declaration)
        })
    }

    fn parse_enum_inner(
        &mut self,
        annotations: Vec<Annotation>,
        visibility: VisibilitySyntax,
    ) -> Result<EnumDecl, Diagnostic> {
        let checkpoint = self.checkpoint();
        let keyword = self.expect("`enum`", |k| matches!(k, TokenKind::Enum))?;
        let name = self.expect_ident("enum name")?;
        self.record_declaration_name(&name);
        let type_params = self.parse_type_params()?;
        let mut end = name.span.end;
        let interfaces = interfaces_only(self.parse_supertypes(&mut end)?)?;
        let where_clause = self.parse_where_clause()?;
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
        let header = scoop_ast::EnumHeader {
            annotations,
            visibility,
            name,
            type_params,
            interfaces,
            where_clause,
            span: Span::new(start, self.tokens[self.pos - 1].span.end),
        };
        if matches!(self.peek().kind, TokenKind::LBrace) {
            self.record_enum_header(&header, checkpoint);
        }
        self.expect("`{`", |k| matches!(k, TokenKind::LBrace))?;
        let body_depth = self.brace_depth();
        let mut variants = Vec::new();
        let mut methods = Vec::new();
        let mut properties = Vec::new();
        let mut nested = Vec::new();
        let mut companion = None;
        let mut interface_implementations = Vec::new();
        let mut saw_member = false;
        let close = loop {
            if matches!(self.peek().kind, TokenKind::RBrace) {
                break self.bump();
            }
            if matches!(self.peek().kind, TokenKind::Eof) {
                return self.missing_token(TokenKind::RBrace, "`}`");
            }
            let start = self.pos;
            let checkpoint = self.checkpoint();
            let annotations = match self.parse_annotations() {
                Ok(annotations) => annotations,
                Err(error) => {
                    self.invalidate_nominal_projection();
                    return Err(error);
                }
            };
            self.begin_nominal_member();
            let parsed = if saw_member || self.enum_member_starts_here() {
                saw_member = true;
                self.parse_enum_member(annotations, companion.is_some())
            } else {
                self.parse_variant(annotations)
                    .map(Box::new)
                    .map(NominalMember::Variant)
            };
            self.end_type_member(&parsed, checkpoint);
            let parsed = parsed.and_then(|member| {
                self.attach_documentation(start, member.span());
                let variant = matches!(member, NominalMember::Variant(_));
                match member {
                    NominalMember::Variant(value) => variants.push(*value),
                    NominalMember::Function(value) => methods.push(*value),
                    NominalMember::Property(value) => properties.push(*value),
                    NominalMember::Nested(value) => nested.push(*value),
                    NominalMember::Companion(value) => companion = Some(*value),
                    NominalMember::InterfaceImplementation(value) => {
                        interface_implementations.push(*value)
                    }
                    NominalMember::Class(_) | NominalMember::Struct(_) => {
                        unreachable!("enum member production")
                    }
                }
                let boundary = if variant {
                    self.expect_variant_end()
                } else {
                    self.expect_statement_end()
                };
                if boundary.is_err() {
                    self.invalidate_nominal_projection();
                }
                boundary
            });
            if let Err(diagnostic) = parsed {
                if self.at_eof() {
                    return Err(diagnostic);
                }
                self.diagnostics.push(diagnostic);
                self.synchronize_body_item(body_depth, start);
            }
        };
        Ok(EnumDecl {
            header,
            variants,
            methods,
            properties,
            nested,
            companion,
            interface_implementations,
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
            let checkpoint = parser.checkpoint();
            let declaration = parser.parse_variant_inner(annotations)?;
            parser.record_declaration_source_span(declaration.span, checkpoint);
            Ok(declaration)
        })
    }

    fn parse_variant_inner(
        &mut self,
        annotations: Vec<Annotation>,
    ) -> Result<VariantDecl, Diagnostic> {
        let name = self.expect_ident("variant name")?;
        self.record_declaration_name(&name);
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
