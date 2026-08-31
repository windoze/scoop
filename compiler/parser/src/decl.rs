//! Top-level declaration parsing: annotations, functions, structs, enums,
//! classes and interfaces.

use scoop_ast::{
    Annotation, ClassDecl, ClassModifier, ConstructorProp, Decl, Diagnostic, EnumDecl, Expr,
    FieldDecl, FunctionBody, FunctionDecl, Ident, InterfaceDecl, MethodModifier, Param, Span,
    StructDecl, TypeParamDecl, Variance, VariantDecl, VariantDeclKind, VariantFieldDecl,
};

use crate::lexer::TokenKind;
use crate::parser::Parser;

/// Where a `fun` declaration appears; drives the modifier and body rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FunctionContext {
    /// Top-level: no modifiers; a body is required (unless `@Intrinsic`).
    TopLevel,
    /// Class / struct / enum body: method modality and `override`
    /// modifiers are parsed here; owner-specific legality is checked
    /// by hir-lower. `abstract` requires a bodyless declaration.
    TypeBody,
    /// Interface body: method signatures only — bodies (default
    /// implementations) are outside the M6 subset.
    Interface,
    /// Block-local declaration: no annotations or member modifiers and a
    /// body is mandatory, like a top-level user function.
    Local,
}

/// The modality / `override` modifiers in front of a member `fun`.
/// `start` is the byte offset of the first modifier keyword, for spans.
#[derive(Debug, Default)]
pub(crate) struct Modifiers {
    pub is_suspend: bool,
    pub suspend_span: Option<Span>,
    pub is_override: bool,
    pub method_modifier: Option<MethodModifier>,
    pub method_modifier_span: Option<Span>,
    pub start: Option<u32>,
}

/// The parsed supertype list of a class: an optional base class with its
/// constructor arguments (`: Base(args)`) plus the implemented interfaces.
#[derive(Debug, Default)]
pub(crate) struct Supertypes {
    pub base_class: Option<(Ident, Vec<Expr>)>,
    pub interfaces: Vec<scoop_ast::TypeRef>,
}

/// Value types implement interfaces only (spec 4.4.3): a supertype with
/// constructor arguments (a base class) is rejected, the rest are
/// interface names.
fn interfaces_only(supertypes: Supertypes) -> Result<Vec<scoop_ast::TypeRef>, Diagnostic> {
    if let Some((base, _)) = supertypes.base_class {
        return Err(Diagnostic::at(
            base.span,
            "value types cannot have a base class (spec 4.4)",
        ));
    }
    Ok(supertypes.interfaces)
}

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
            TokenKind::Suspend => {
                let suspend = self.bump();
                if matches!(self.peek().kind, TokenKind::Suspend) {
                    return Err(Diagnostic::at(
                        self.peek().span,
                        "duplicate `suspend` modifier on top-level function",
                    ));
                }
                if !matches!(self.peek().kind, TokenKind::Fun) {
                    return Err(Diagnostic::at(
                        suspend.span,
                        "`suspend` modifier is only allowed on function declarations",
                    ));
                }
                let modifiers = Modifiers {
                    is_suspend: true,
                    suspend_span: Some(suspend.span),
                    start: Some(suspend.span.start),
                    ..Modifiers::default()
                };
                Ok(Decl::Function(self.parse_function(
                    Vec::new(),
                    modifiers,
                    FunctionContext::TopLevel,
                )?))
            }
            TokenKind::Fun => Ok(Decl::Function(self.parse_function(
                Vec::new(),
                Modifiers::default(),
                FunctionContext::TopLevel,
            )?)),
            TokenKind::Struct => Ok(Decl::Struct(self.parse_struct()?)),
            TokenKind::Enum => Ok(Decl::Enum(self.parse_enum()?)),
            TokenKind::Class => Ok(Decl::Class(self.parse_class(ClassModifier::Final, None)?)),
            TokenKind::Ident(text) if text == "open" || text == "abstract" => {
                let modifier = if text == "open" {
                    ClassModifier::Open
                } else {
                    ClassModifier::Abstract
                };
                let keyword = self.bump();
                Ok(Decl::Class(self.parse_class(modifier, Some(keyword.span))?))
            }
            TokenKind::Interface => Ok(Decl::Interface(self.parse_interface()?)),
            TokenKind::At => {
                let annotations = self.parse_annotations()?;
                let mut modifiers = Modifiers::default();
                if matches!(self.peek().kind, TokenKind::Suspend) {
                    let suspend = self.bump();
                    modifiers.is_suspend = true;
                    modifiers.suspend_span = Some(suspend.span);
                    modifiers.start = Some(suspend.span.start);
                    if matches!(self.peek().kind, TokenKind::Suspend) {
                        return Err(Diagnostic::at(
                            self.peek().span,
                            "duplicate `suspend` modifier on top-level function",
                        ));
                    }
                }
                if !matches!(self.peek().kind, TokenKind::Fun) {
                    return Err(Diagnostic::at(
                        self.peek().span,
                        "annotations are only allowed on function declarations (milestone M4)",
                    ));
                }
                Ok(Decl::Function(self.parse_function(
                    annotations,
                    modifiers,
                    FunctionContext::TopLevel,
                )?))
            }
            TokenKind::Ident(text) if text == "sealed" => Err(Diagnostic::at(
                self.peek().span,
                "`sealed` classes are not supported yet (milestone M6)",
            )),
            TokenKind::Ident(text) if text == "object" => Err(Diagnostic::at(
                self.peek().span,
                "`object` declarations are not supported yet (milestone M6)",
            )),
            _ => self.unexpected("`fun`, `struct`, `enum`, `class` or `interface`"),
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

    /// `(open|final|abstract|override)* fun <T, ...>? (<receiver>.)?<name>(<param>, ...)?: <ret>? <body>?`
    /// — the type parameter list sits between `fun` and the name (a `<`
    /// right after `fun` is unambiguous here), parameters carry mandatory
    /// type annotations, and the return type defaults to `Unit` when
    /// absent. The body is a block or an expression body (`= expr`);
    /// bodyless declarations (`FunctionBody::None`) are `abstract fun`,
    /// interface method signatures, and `@Intrinsic` functions (spec 13.1).
    /// A non-abstract, non-intrinsic function outside an interface must
    /// have a body.
    pub(crate) fn parse_function(
        &mut self,
        annotations: Vec<Annotation>,
        modifiers: Modifiers,
        context: FunctionContext,
    ) -> Result<FunctionDecl, Diagnostic> {
        let fun = self.expect("`fun`", |k| matches!(k, TokenKind::Fun))?;
        let type_params = self.parse_type_params()?;
        let receiver_start = self.pos;
        let receiver_ty = match self.parse_type_ref() {
            Ok(ty) if matches!(self.peek().kind, TokenKind::Dot) => {
                self.bump();
                Some(ty)
            }
            _ => {
                self.pos = receiver_start;
                None
            }
        };
        if let Some(receiver) = &receiver_ty
            && context != FunctionContext::TopLevel
        {
            return Err(Diagnostic::at(
                receiver.span,
                "extension functions may only be declared at top level",
            ));
        }
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
            TokenKind::LBrace | TokenKind::Equal
                if modifiers.method_modifier == Some(MethodModifier::Abstract) =>
            {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "`abstract` functions must not have a body",
                ));
            }
            TokenKind::LBrace | TokenKind::Equal if context == FunctionContext::Interface => {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "interface method bodies are not supported yet (milestone M6)",
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
            _ if !annotations.is_empty()
                || modifiers.method_modifier == Some(MethodModifier::Abstract)
                || context == FunctionContext::Interface =>
            {
                (FunctionBody::None, signature_end)
            }
            _ => return self.unexpected("`{` or `=`"),
        };
        let start = annotations
            .first()
            .map(|annotation| annotation.span.start)
            .or(modifiers.start)
            .unwrap_or(fun.span.start);
        let modifier = modifiers.method_modifier.unwrap_or_else(|| {
            if context == FunctionContext::Interface {
                MethodModifier::Abstract
            } else if modifiers.is_override {
                // Kotlin-compatible rule (spec 9.1): overrides stay
                // open unless explicitly closed with `final`.
                MethodModifier::Open
            } else {
                MethodModifier::Final
            }
        });
        Ok(FunctionDecl {
            annotations,
            is_suspend: modifiers.is_suspend,
            is_override: modifiers.is_override,
            modifier,
            receiver_ty,
            name,
            type_params,
            params,
            return_ty,
            body,
            span: Span::new(start, end),
        })
    }

    /// `struct <name><T, ...>?(val <field>: <type>, ...) (: <interface>, ...)?
    /// ({ <fun>, ... })?` — the type parameter list sits between the name
    /// and the constructor `(`, like `enum` (a `<` right after the name is
    /// unambiguous here). M6 adds an optional interface list (spec 4.4.3)
    /// and an optional member body holding member functions (value
    /// receiver).
    fn parse_struct(&mut self) -> Result<StructDecl, Diagnostic> {
        let keyword = self.expect("`struct`", |k| matches!(k, TokenKind::Struct))?;
        let name = self.expect_ident("struct name")?;
        let type_params = self.parse_type_params()?;
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
        let mut end = close.span.end;
        let interfaces = interfaces_only(self.parse_supertypes(&mut end)?)?;
        let methods = if matches!(self.peek().kind, TokenKind::LBrace) {
            let (methods, body_end) = self.parse_member_body(FunctionContext::TypeBody)?;
            end = body_end;
            methods
        } else {
            Vec::new()
        };
        Ok(StructDecl {
            name,
            type_params,
            fields,
            interfaces,
            methods,
            span: Span::new(keyword.span.start, end),
        })
    }

    /// `(open|abstract)? class <name>(<ctor prop>, ...)? (: <supertypes>)?
    /// ({ <fun>, ... })?` (spec 9.1). Constructor properties must be
    /// declared with `val` / `var`. The first supertype may carry
    /// constructor arguments — that makes it the base class
    /// (`Base(args)`); the rest are interfaces. The body holds member
    /// functions only in M6.
    fn parse_class(
        &mut self,
        modifier: ClassModifier,
        modifier_span: Option<Span>,
    ) -> Result<ClassDecl, Diagnostic> {
        let keyword = self.expect("`class`", |k| matches!(k, TokenKind::Class))?;
        let name = self.expect_ident("class name")?;
        let mut end = name.span.end;
        let mut constructor = Vec::new();
        if matches!(self.peek().kind, TokenKind::LParen) {
            self.bump(); // `(`
            if !matches!(self.peek().kind, TokenKind::RParen) {
                loop {
                    let prop_keyword = self.peek().clone();
                    let mutable = match prop_keyword.kind {
                        TokenKind::Val => false,
                        TokenKind::Var => true,
                        _ => {
                            return Err(Diagnostic::at(
                                prop_keyword.span,
                                "class constructor parameters must be properties declared with `val` or `var`",
                            ));
                        }
                    };
                    self.pos += 1;
                    let prop_name = self.expect_ident("property name")?;
                    self.expect("`:`", |k| matches!(k, TokenKind::Colon))?;
                    let ty = self.parse_type_ref()?;
                    constructor.push(ConstructorProp {
                        mutable,
                        span: Span::new(prop_keyword.span.start, ty.span.end),
                        name: prop_name,
                        ty,
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
        }
        let supertypes = self.parse_supertypes(&mut end)?;
        let methods = if matches!(self.peek().kind, TokenKind::LBrace) {
            let (methods, body_end) = self.parse_member_body(FunctionContext::TypeBody)?;
            end = body_end;
            methods
        } else {
            Vec::new()
        };
        let start = modifier_span
            .map(|span| span.start)
            .unwrap_or(keyword.span.start);
        Ok(ClassDecl {
            modifier,
            name,
            constructor,
            base_class: supertypes.base_class,
            interfaces: supertypes.interfaces,
            methods,
            span: Span::new(start, end),
        })
    }

    /// `: Base(args), I1, I2` — a no-op when the next token is not `:`.
    /// Constructor arguments are only allowed on the first supertype (the
    /// base class); whether a bare name is a class or an interface is
    /// HIR's call. `end` advances past the last supertype.
    fn parse_supertypes(&mut self, end: &mut u32) -> Result<Supertypes, Diagnostic> {
        let mut supertypes = Supertypes::default();
        if !matches!(self.peek().kind, TokenKind::Colon) {
            return Ok(supertypes);
        }
        self.bump(); // `:`
        loop {
            let super_name = self.expect_ident("base class or interface name")?;
            *end = super_name.span.end;
            if matches!(self.peek().kind, TokenKind::LParen) {
                if supertypes.base_class.is_some() || !supertypes.interfaces.is_empty() {
                    return Err(Diagnostic::at(
                        self.peek().span,
                        "constructor arguments are only allowed on the base class (the first supertype)",
                    ));
                }
                let (args, args_end) = self.parse_args()?;
                supertypes.base_class = Some((super_name, args));
                *end = args_end;
            } else {
                let supertype = self.parse_named_type_ref_tail(super_name)?;
                *end = supertype.span.end;
                supertypes.interfaces.push(supertype);
            }
            if matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        Ok(supertypes)
    }

    /// `interface <name> { <fun signature>, ... }` — method signatures
    /// only in M6 (no properties, no default implementations).
    fn parse_interface(&mut self) -> Result<InterfaceDecl, Diagnostic> {
        let keyword = self.expect("`interface`", |k| matches!(k, TokenKind::Interface))?;
        let name = self.expect_ident("interface name")?;
        let type_params = self.parse_interface_type_params()?;
        let (methods, end) = self.parse_member_body(FunctionContext::Interface)?;
        Ok(InterfaceDecl {
            name,
            type_params,
            methods,
            span: Span::new(keyword.span.start, end),
        })
    }

    /// `{ <member fun>, ... }` — the `{` is the current token. Only `fun`
    /// declarations (with optional `override` / `abstract`) are members in
    /// M6; everything else gets a dedicated "not supported" diagnostic.
    /// Returns the methods and the closing brace's end offset.
    fn parse_member_body(
        &mut self,
        context: FunctionContext,
    ) -> Result<(Vec<FunctionDecl>, u32), Diagnostic> {
        self.bump(); // `{`
        let body_depth = self.brace_depth();
        let mut methods = Vec::new();
        let close = loop {
            if matches!(self.peek().kind, TokenKind::RBrace) {
                break self.bump();
            }
            if matches!(self.peek().kind, TokenKind::Eof) {
                return self.unexpected("`}`");
            }
            let start = self.pos;
            let parsed = match &self.peek().kind {
                TokenKind::Val | TokenKind::Var => Err(Diagnostic::at(
                    self.peek().span,
                    "member properties are not supported yet (milestone M6)",
                )),
                TokenKind::Struct | TokenKind::Enum | TokenKind::Class | TokenKind::Interface => {
                    Err(Diagnostic::at(
                        self.peek().span,
                        "nested type declarations are not supported yet (milestone M6)",
                    ))
                }
                TokenKind::Ident(text) if text == "init" => Err(Diagnostic::at(
                    self.peek().span,
                    "`init` blocks are not supported yet (milestone M6)",
                )),
                TokenKind::Ident(text) if text == "constructor" => Err(Diagnostic::at(
                    self.peek().span,
                    "secondary constructors are not supported yet (milestone M6)",
                )),
                TokenKind::Ident(text) if text == "companion" => Err(Diagnostic::at(
                    self.peek().span,
                    "companion objects are not supported yet (milestone M6)",
                )),
                TokenKind::Ident(text) if text == "object" => Err(Diagnostic::at(
                    self.peek().span,
                    "`object` declarations are not supported yet (milestone M6)",
                )),
                _ => self.parse_member_function(context),
            };
            match parsed {
                Ok(method) => {
                    methods.push(method);
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
        Ok((methods, close.span.end))
    }

    /// `(open|final|abstract|override|suspend)* fun ...` — a member function.
    /// Modifiers may appear in any order; modality keywords are
    /// mutually exclusive and every keyword may appear at most once.
    fn parse_member_function(
        &mut self,
        context: FunctionContext,
    ) -> Result<FunctionDecl, Diagnostic> {
        let mut modifiers = Modifiers::default();
        loop {
            let token = self.peek().clone();
            if matches!(token.kind, TokenKind::Suspend) {
                if modifiers.is_suspend {
                    return Err(Diagnostic::at(
                        token.span,
                        "duplicate `suspend` modifier on member function",
                    ));
                }
                modifiers.is_suspend = true;
                let keyword = self.bump();
                modifiers.suspend_span = Some(keyword.span);
                modifiers.start = modifiers.start.or(Some(keyword.span.start));
                continue;
            }
            let TokenKind::Ident(text) = &token.kind else {
                break;
            };
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
        if context == FunctionContext::Interface
            && matches!(
                modifiers.method_modifier,
                Some(MethodModifier::Final | MethodModifier::Open)
            )
        {
            let modifier = match modifiers.method_modifier.expect("matched above") {
                MethodModifier::Final => "final",
                MethodModifier::Open => "open",
                MethodModifier::Abstract => unreachable!(),
            };
            return Err(Diagnostic::at(
                modifiers.method_modifier_span.expect("explicit modality"),
                format!("`{modifier}` modifier is not allowed on interface methods"),
            ));
        }
        if modifiers.is_suspend && !matches!(self.peek().kind, TokenKind::Fun) {
            return Err(Diagnostic::at(
                modifiers
                    .suspend_span
                    .expect("suspend modifier has a source span"),
                "`suspend` modifier is only allowed on function declarations",
            ));
        }
        self.parse_function(Vec::new(), modifiers, context)
    }

    /// `enum <name><T, ...>? (: <interface>, ...)? { <variant>, ...
    /// (<fun>, ...)? }` (spec 4.2). The interface list (spec 4.4.3) sits
    /// after the name / type parameters, before `{`. Variants separate
    /// like statements, with `,` in place of `;`; M6 adds member
    /// functions after the variants.
    fn parse_enum(&mut self) -> Result<EnumDecl, Diagnostic> {
        let keyword = self.expect("`enum`", |k| matches!(k, TokenKind::Enum))?;
        let name = self.expect_ident("enum name")?;
        let type_params = self.parse_type_params()?;
        let mut end = name.span.end;
        let interfaces = interfaces_only(self.parse_supertypes(&mut end)?)?;
        self.expect("`{`", |k| matches!(k, TokenKind::LBrace))?;
        let body_depth = self.brace_depth();
        let mut variants = Vec::new();
        let mut methods = Vec::new();
        let close = loop {
            if matches!(self.peek().kind, TokenKind::RBrace) {
                break self.bump();
            }
            if matches!(self.peek().kind, TokenKind::Eof) {
                return self.unexpected("`}`");
            }
            let start = self.pos;
            let parsed = match &self.peek().kind {
                TokenKind::Fun | TokenKind::Suspend => self
                    .parse_member_function(FunctionContext::TypeBody)
                    .map(|method| methods.push(method))
                    .and_then(|()| self.expect_statement_end()),
                TokenKind::Ident(text)
                    if matches!(text.as_str(), "override" | "abstract" | "open" | "final") =>
                {
                    self.parse_member_function(FunctionContext::TypeBody)
                        .map(|method| methods.push(method))
                        .and_then(|()| self.expect_statement_end())
                }
                TokenKind::Ident(text) if text == "init" => Err(Diagnostic::at(
                    self.peek().span,
                    "`init` blocks are not supported yet (milestone M6)",
                )),
                _ => self
                    .parse_variant()
                    .map(|variant| variants.push(variant))
                    .and_then(|()| self.expect_variant_end()),
            };
            if let Err(diagnostic) = parsed {
                if self.at_eof() {
                    return Err(diagnostic);
                }
                self.diagnostics.push(diagnostic);
                self.synchronize_body_item(body_depth, start);
            }
        };
        Ok(EnumDecl {
            name,
            type_params,
            variants,
            interfaces,
            methods,
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

    /// Interface declaration parameters additionally accept declaration-site
    /// `in` / `out` variance. They remain identifiers elsewhere in the grammar.
    fn parse_interface_type_params(&mut self) -> Result<Vec<TypeParamDecl>, Diagnostic> {
        let mut type_params = Vec::new();
        if !matches!(self.peek().kind, TokenKind::Less) {
            return Ok(type_params);
        }
        self.bump();
        loop {
            let variance_token = self.peek().clone();
            let variance = match &variance_token.kind {
                TokenKind::Ident(text) if text == "in" => {
                    self.bump();
                    Variance::In
                }
                TokenKind::Ident(text) if text == "out" => {
                    self.bump();
                    Variance::Out
                }
                _ => Variance::Invariant,
            };
            let name = self.expect_ident("type parameter name")?;
            let start = if variance == Variance::Invariant {
                name.span.start
            } else {
                variance_token.span.start
            };
            type_params.push(TypeParamDecl {
                span: Span::new(start, name.span.end),
                name,
                variance,
            });
            if matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        self.expect("`>`", |k| matches!(k, TokenKind::Greater))?;
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
