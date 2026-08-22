//! Top-level declaration parsing: annotations, functions, structs, enums.

use scoop_ast::{
    Annotation, Decl, Diagnostic, EnumDecl, Expr, FieldDecl, FunctionBody, FunctionDecl, Ident,
    Param, Span, StructDecl, VariantDecl, VariantDeclKind, VariantFieldDecl,
};

use crate::lexer::TokenKind;
use crate::parser::Parser;

/// M4 variant field defaults are literal constant expressions only
/// (DESIGN.md 5.4).
fn is_constant(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::IntLiteral { .. }
            | Expr::StringLiteral { .. }
            | Expr::BoolLiteral { .. }
            | Expr::UnitLiteral { .. }
    )
}

impl Parser {
    pub(crate) fn parse_decl(&mut self) -> Result<Decl, Diagnostic> {
        match &self.peek().kind {
            TokenKind::Fun => Ok(Decl::Function(self.parse_function(Vec::new())?)),
            TokenKind::Struct => Ok(Decl::Struct(self.parse_struct()?)),
            TokenKind::Enum => Ok(Decl::Enum(self.parse_enum()?)),
            TokenKind::At => {
                let annotations = self.parse_annotations()?;
                if !matches!(self.peek().kind, TokenKind::Fun) {
                    return Err(Diagnostic::at(
                        self.peek().span,
                        "annotations are only allowed on function declarations (milestone M4)",
                    ));
                }
                Ok(Decl::Function(self.parse_function(annotations)?))
            }
            _ => self.unexpected("`fun`, `struct` or `enum`"),
        }
    }

    /// M4 supports a single annotation per function, and only
    /// `@Intrinsic("name")` (DESIGN.md 1.3).
    fn parse_annotations(&mut self) -> Result<Vec<Annotation>, Diagnostic> {
        let mut annotations = Vec::new();
        while matches!(self.peek().kind, TokenKind::At) {
            let at = self.bump();
            if !annotations.is_empty() {
                return Err(Diagnostic::at(
                    at.span,
                    "only a single annotation is supported (milestone M4)",
                ));
            }
            let name = self.expect_ident("annotation name")?;
            if name.text != "Intrinsic" {
                return Err(Diagnostic::at(
                    Span::new(at.span.start, name.span.end),
                    "annotations are not supported yet (milestone M4)",
                ));
            }
            let (value, end) = self.parse_intrinsic_argument()?;
            annotations.push(Annotation {
                name,
                value: Some(value),
                span: Span::new(at.span.start, end),
            });
        }
        Ok(annotations)
    }

    /// The `("name")` argument list of `@Intrinsic`: exactly one string
    /// literal.
    fn parse_intrinsic_argument(&mut self) -> Result<(String, u32), Diagnostic> {
        let error =
            |span: Span| Diagnostic::at(span, "`@Intrinsic` requires exactly one string argument");
        let open = self.peek().clone();
        if !matches!(open.kind, TokenKind::LParen) {
            return Err(error(open.span));
        }
        self.pos += 1;
        let arg = self.peek().clone();
        let TokenKind::Str(value) = arg.kind else {
            return Err(error(arg.span));
        };
        self.pos += 1;
        let close = self.peek().clone();
        if !matches!(close.kind, TokenKind::RParen) {
            return Err(error(close.span));
        }
        self.pos += 1;
        Ok((value, close.span.end))
    }

    /// `fun <T, ...>? <name>(<param>, ...)?: <ret>? <body>` — the type
    /// parameter list sits between `fun` and the name (a `<` right after
    /// `fun` is unambiguous here), parameters carry mandatory type
    /// annotations, and the return type defaults to `Unit` when absent.
    /// The body is a block or an expression body (`= expr`); a function
    /// annotated with `@Intrinsic` must omit it (spec 13.1).
    fn parse_function(&mut self, annotations: Vec<Annotation>) -> Result<FunctionDecl, Diagnostic> {
        let fun = self.expect("`fun`", |k| matches!(k, TokenKind::Fun))?;
        let type_params = self.parse_type_params()?;
        let name = self.expect_ident("function name")?;
        self.expect("`(`", |k| matches!(k, TokenKind::LParen))?;
        let mut params = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RParen) {
            loop {
                let param_name = self.expect_ident("parameter name")?;
                self.expect("`:`", |k| matches!(k, TokenKind::Colon))?;
                let ty = self.parse_type_ref()?;
                params.push(Param {
                    span: Span::new(param_name.span.start, ty.span.end),
                    name: param_name,
                    ty,
                });
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        let params_close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
        let return_ty = if matches!(self.peek().kind, TokenKind::Colon) {
            self.bump();
            Some(self.parse_type_ref()?)
        } else {
            None
        };
        let signature_end = return_ty
            .as_ref()
            .map(|ty| ty.span.end)
            .unwrap_or(params_close.span.end);
        let (body, end) = match self.peek().kind {
            TokenKind::LBrace | TokenKind::Equal if !annotations.is_empty() => {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "functions annotated with `@Intrinsic` must not have a body (spec 13.1)",
                ));
            }
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                let end = block.span.end;
                (FunctionBody::Block(block), end)
            }
            TokenKind::Equal => {
                self.bump();
                let expr = self.parse_expr()?;
                let end = expr.span().end;
                (FunctionBody::Expr(Box::new(expr)), end)
            }
            _ if !annotations.is_empty() => {
                // Spec 13.1: an `@Intrinsic` function omits its body. The
                // AST has no bodiless `FunctionBody` variant, so this is
                // an empty block; `annotations` is what HIR keys on.
                let span = Span::new(signature_end, signature_end);
                (
                    FunctionBody::Block(scoop_ast::Block {
                        statements: Vec::new(),
                        span,
                    }),
                    signature_end,
                )
            }
            _ => return self.unexpected("`{` or `=`"),
        };
        let start = annotations
            .first()
            .map(|annotation| annotation.span.start)
            .unwrap_or(fun.span.start);
        Ok(FunctionDecl {
            annotations,
            name,
            type_params,
            params,
            return_ty,
            body,
            span: Span::new(start, end),
        })
    }

    /// `struct <name>(val <field>: <type>, ...)` — M2 structs are a primary
    /// constructor only: all-`val` fields, no defaults, no member body.
    fn parse_struct(&mut self) -> Result<StructDecl, Diagnostic> {
        let keyword = self.expect("`struct`", |k| matches!(k, TokenKind::Struct))?;
        let name = self.expect_ident("struct name")?;
        self.expect("`(`", |k| matches!(k, TokenKind::LParen))?;
        let mut fields = Vec::new();
        if matches!(self.peek().kind, TokenKind::RParen) {
            // A fieldless struct is outside the M2 subset.
            return self.unexpected("field declaration");
        }
        loop {
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
            if matches!(self.peek().kind, TokenKind::Equal) {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "field default values are not supported yet (milestone M3)",
                ));
            }
            fields.push(FieldDecl {
                span: Span::new(val.span.start, ty.span.end),
                name: field_name,
                ty,
            });
            if matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        let close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
        if matches!(self.peek().kind, TokenKind::LBrace) {
            return Err(Diagnostic::at(
                self.peek().span,
                "struct member declarations are not supported yet (milestone M3)",
            ));
        }
        Ok(StructDecl {
            name,
            fields,
            span: Span::new(keyword.span.start, close.span.end),
        })
    }

    /// `enum <name><T, ...>? { <variant>, ... }` (spec 4.2). Variants
    /// separate like statements, with `,` in place of `;`. Member
    /// functions and `init` blocks are outside the M4 subset.
    fn parse_enum(&mut self) -> Result<EnumDecl, Diagnostic> {
        let keyword = self.expect("`enum`", |k| matches!(k, TokenKind::Enum))?;
        let name = self.expect_ident("enum name")?;
        let type_params = self.parse_type_params()?;
        self.expect("`{`", |k| matches!(k, TokenKind::LBrace))?;
        let mut variants = Vec::new();
        let close = loop {
            match &self.peek().kind {
                TokenKind::RBrace => break self.bump(),
                TokenKind::Eof => return self.unexpected("`}`"),
                TokenKind::Fun => {
                    return Err(Diagnostic::at(
                        self.peek().span,
                        "enum member functions are not supported yet (milestone M4)",
                    ));
                }
                TokenKind::Ident(text) if text == "init" => {
                    return Err(Diagnostic::at(
                        self.peek().span,
                        "enum `init` blocks are not supported yet (milestone M4)",
                    ));
                }
                _ => {
                    variants.push(self.parse_variant()?);
                    self.expect_variant_end()?;
                }
            }
        };
        Ok(EnumDecl {
            name,
            type_params,
            variants,
            span: Span::new(keyword.span.start, close.span.end),
        })
    }

    /// `<T, ...>`; empty when the next token is not `<`.
    fn parse_type_params(&mut self) -> Result<Vec<Ident>, Diagnostic> {
        let mut type_params = Vec::new();
        if matches!(self.peek().kind, TokenKind::Less) {
            self.bump();
            loop {
                type_params.push(self.expect_ident("type parameter name")?);
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                } else {
                    break;
                }
            }
            self.expect("`>`", |k| matches!(k, TokenKind::Greater))?;
        }
        Ok(type_params)
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
    fn parse_variant(&mut self) -> Result<VariantDecl, Diagnostic> {
        let name = self.expect_ident("variant name")?;
        let start = name.span.start;
        let (kind, end) = match self.peek().kind {
            TokenKind::LParen
                if matches!(
                    self.tokens.get(self.pos + 1).map(|token| &token.kind),
                    Some(TokenKind::Val | TokenKind::Var)
                ) =>
            {
                let (fields, end) = self.parse_constructor_fields()?;
                (VariantDeclKind::Constructor(fields), end)
            }
            TokenKind::LParen => {
                self.bump(); // `(`
                let mut types = Vec::new();
                loop {
                    types.push(self.parse_type_ref()?);
                    if matches!(self.peek().kind, TokenKind::Comma) {
                        self.bump();
                    } else {
                        break;
                    }
                }
                let close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
                (VariantDeclKind::Positional(types), close.span.end)
            }
            TokenKind::LBrace => {
                let (fields, end) = self.parse_named_fields()?;
                (VariantDeclKind::Named(fields), end)
            }
            _ => (VariantDeclKind::Unit, name.span.end),
        };
        Ok(VariantDecl {
            name,
            kind,
            span: Span::new(start, end),
        })
    }

    /// `(val f: T = default, ...)` — the `(` is the current token and the
    /// caller has checked that a `val`/`var` follows it.
    fn parse_constructor_fields(&mut self) -> Result<(Vec<VariantFieldDecl>, u32), Diagnostic> {
        self.bump(); // `(`
        let mut fields = Vec::new();
        loop {
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
            let mut end = ty.span.end;
            let default = if matches!(self.peek().kind, TokenKind::Equal) {
                self.bump();
                let expr = self.parse_expr()?;
                if !is_constant(&expr) {
                    return Err(Diagnostic::at(
                        expr.span(),
                        "only constant expressions are allowed as variant field defaults (milestone M4)",
                    ));
                }
                end = expr.span().end;
                Some(expr)
            } else {
                None
            };
            fields.push(VariantFieldDecl {
                span: Span::new(val.span.start, end),
                name: field_name,
                ty,
                default,
            });
            if matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
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
                span: Span::new(field_name.span.start, ty.span.end),
                name: field_name,
                ty,
                default: None,
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
