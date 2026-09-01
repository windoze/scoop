//! Top-level declaration parsing: annotations, functions, structs, enums,
//! classes and interfaces.

use scoop_ast::{
    Annotation, AnnotationArg, AnnotationLiteral, ClassConstructorDecl, ClassDecl, ClassModifier,
    ConstructorProp, Decl, Diagnostic, EnumDecl, Expr, FieldDecl, FunctionBody, FunctionDecl,
    GlobalDecl, InterfaceDecl, MethodModifier, OperatorModifier, Param, Span, StructDecl,
    StructRepresentationDecl, TypeBound, TypeConstraint, TypeParamDecl, TypeParamKindBound,
    Variance, VariantDecl, VariantDeclKind, VariantFieldDecl, WhereClause,
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
    pub operator: Option<OperatorModifier>,
    pub method_modifier: Option<MethodModifier>,
    pub method_modifier_span: Option<Span>,
    pub start: Option<u32>,
}

/// The parsed supertype list of a class: an optional base class with its
/// constructor arguments (`: Base(args)`) plus the implemented interfaces.
#[derive(Debug, Default)]
pub(crate) struct Supertypes {
    pub base_class: Option<(scoop_ast::TypeRef, Vec<Expr>)>,
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
            TokenKind::Suspend | TokenKind::Fun => Ok(Decl::Function(
                self.parse_non_member_function(Vec::new(), FunctionContext::TopLevel)?,
            )),
            TokenKind::Val | TokenKind::Var => Ok(Decl::Global(self.parse_global(Vec::new())?)),
            TokenKind::Struct => Ok(Decl::Struct(self.parse_struct(Vec::new())?)),
            TokenKind::Enum => Ok(Decl::Enum(self.parse_enum(Vec::new())?)),
            TokenKind::Class => Ok(Decl::Class(self.parse_class(
                Vec::new(),
                ClassModifier::Final,
                None,
            )?)),
            TokenKind::Ident(text) if text == "operator" => Ok(Decl::Function(
                self.parse_non_member_function(Vec::new(), FunctionContext::TopLevel)?,
            )),
            TokenKind::Ident(text) if text == "open" || text == "abstract" => {
                let modifier = if text == "open" {
                    ClassModifier::Open
                } else {
                    ClassModifier::Abstract
                };
                let keyword = self.bump();
                Ok(Decl::Class(self.parse_class(
                    Vec::new(),
                    modifier,
                    Some(keyword.span),
                )?))
            }
            TokenKind::Interface => Ok(Decl::Interface(self.parse_interface(Vec::new())?)),
            TokenKind::At => {
                let annotations = self.parse_annotations()?;
                match &self.peek().kind {
                    TokenKind::Fun | TokenKind::Suspend => Ok(Decl::Function(
                        self.parse_non_member_function(annotations, FunctionContext::TopLevel)?,
                    )),
                    TokenKind::Ident(text) if text == "operator" => Ok(Decl::Function(
                        self.parse_non_member_function(annotations, FunctionContext::TopLevel)?,
                    )),
                    TokenKind::Val | TokenKind::Var => {
                        Ok(Decl::Global(self.parse_global(annotations)?))
                    }
                    TokenKind::Struct => Ok(Decl::Struct(self.parse_struct(annotations)?)),
                    TokenKind::Enum => Ok(Decl::Enum(self.parse_enum(annotations)?)),
                    TokenKind::Class => Ok(Decl::Class(self.parse_class(
                        annotations,
                        ClassModifier::Final,
                        None,
                    )?)),
                    TokenKind::Interface => Ok(Decl::Interface(self.parse_interface(annotations)?)),
                    _ => Err(Diagnostic::at(
                        self.peek().span,
                        "annotations are only allowed on declarations or safety blocks",
                    )),
                }
            }
            TokenKind::Ident(text) if text == "sealed" => Err(Diagnostic::at(
                self.peek().span,
                "`sealed` classes are not supported yet (milestone M6)",
            )),
            TokenKind::Ident(text) if text == "object" => Err(Diagnostic::at(
                self.peek().span,
                "`object` declarations are not supported yet (milestone M6)",
            )),
            _ => self.unexpected("`fun`, `val`, `var`, `struct`, `enum`, `class` or `interface`"),
        }
    }

    /// Parse the modifiers accepted syntactically on top-level and local
    /// functions. Target/signature legality of `operator` belongs to HIR.
    pub(crate) fn parse_non_member_function(
        &mut self,
        annotations: Vec<Annotation>,
        context: FunctionContext,
    ) -> Result<FunctionDecl, Diagnostic> {
        let mut modifiers = Modifiers::default();
        loop {
            let token = self.peek().clone();
            match &token.kind {
                TokenKind::Suspend => {
                    if modifiers.is_suspend {
                        let location = if context == FunctionContext::TopLevel {
                            "top-level "
                        } else {
                            ""
                        };
                        return Err(Diagnostic::at(
                            token.span,
                            format!("duplicate `suspend` modifier on {location}function"),
                        ));
                    }
                    self.bump();
                    modifiers.is_suspend = true;
                    modifiers.suspend_span = Some(token.span);
                    modifiers.start = modifiers.start.or(Some(token.span.start));
                }
                TokenKind::Ident(text) if text == "operator" => {
                    if modifiers.operator.is_some() {
                        return Err(Diagnostic::at(
                            token.span,
                            "duplicate `operator` modifier on function",
                        ));
                    }
                    self.bump();
                    modifiers.operator = Some(OperatorModifier { span: token.span });
                    modifiers.start = modifiers.start.or(Some(token.span.start));
                }
                _ => break,
            }
        }
        if !matches!(self.peek().kind, TokenKind::Fun) {
            if modifiers.is_suspend && modifiers.operator.is_none() {
                return Err(Diagnostic::at(
                    modifiers
                        .suspend_span
                        .expect("suspend modifier has a source span"),
                    "`suspend` modifier is only allowed on function declarations",
                ));
            }
            let span = modifiers
                .suspend_span
                .or(modifiers.operator.map(|modifier| modifier.span))
                .unwrap_or(self.peek().span);
            return Err(Diagnostic::at(
                span,
                "function modifiers must be followed by `fun`",
            ));
        }
        self.parse_function(annotations, modifiers, context)
    }

    fn parse_global(&mut self, annotations: Vec<Annotation>) -> Result<GlobalDecl, Diagnostic> {
        let keyword = self.bump();
        let mutable = matches!(keyword.kind, TokenKind::Var);
        let name = self.expect_ident("global name")?;
        self.expect("`:`", |kind| matches!(kind, TokenKind::Colon))?;
        let ty = self.parse_type_ref()?;
        let init = if matches!(self.peek().kind, TokenKind::Equal) {
            self.bump();
            Some(self.parse_expr()?)
        } else {
            None
        };
        let end = init.as_ref().map_or(ty.span.end, |expr| expr.span().end);
        Ok(GlobalDecl {
            annotations,
            mutable,
            name,
            ty,
            init,
            span: Span::new(keyword.span.start, end),
        })
    }

    /// Parse compiler annotations without assigning them language semantics.
    /// Target, schema and coexistence checks belong to HIR (M12 design 1.2).
    pub(crate) fn parse_annotations(&mut self) -> Result<Vec<Annotation>, Diagnostic> {
        let mut annotations = Vec::new();
        while matches!(self.peek().kind, TokenKind::At) {
            let at = self.bump();
            let name = self.expect_ident("annotation name")?;
            let (args, end) = self.parse_annotation_arguments(name.span.end)?;
            annotations.push(Annotation {
                name,
                args,
                span: Span::new(at.span.start, end),
            });
        }
        Ok(annotations)
    }

    fn parse_annotation_arguments(
        &mut self,
        marker_end: u32,
    ) -> Result<(Vec<AnnotationArg>, u32), Diagnostic> {
        if !matches!(self.peek().kind, TokenKind::LParen) {
            return Ok((Vec::new(), marker_end));
        }
        self.bump();
        let mut args = Vec::new();
        let mut saw_named = false;
        if !matches!(self.peek().kind, TokenKind::RParen) {
            loop {
                let start = self.peek().span.start;
                let name = if matches!(self.peek().kind, TokenKind::Ident(_))
                    && matches!(self.tokens[self.pos + 1].kind, TokenKind::Equal)
                {
                    let name = self.expect_ident("annotation argument name")?;
                    self.bump();
                    saw_named = true;
                    Some(name)
                } else {
                    if saw_named {
                        return Err(Diagnostic::at(
                            self.peek().span,
                            "positional annotation arguments must precede named arguments",
                        ));
                    }
                    None
                };
                let token = self.bump();
                let value = match token.kind {
                    TokenKind::Str(value) => AnnotationLiteral::String(value),
                    TokenKind::Int(value) => AnnotationLiteral::Int(value),
                    TokenKind::True => AnnotationLiteral::Boolean(true),
                    TokenKind::False => AnnotationLiteral::Boolean(false),
                    _ => {
                        return Err(Diagnostic::at(
                            token.span,
                            "annotation argument must be a string, integer or boolean literal",
                        ));
                    }
                };
                args.push(AnnotationArg {
                    name,
                    value,
                    span: Span::new(start, token.span.end),
                });
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                    if matches!(self.peek().kind, TokenKind::RParen) {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        let close = self.expect("`)`", |kind| matches!(kind, TokenKind::RParen))?;
        Ok((args, close.span.end))
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
        let where_clause = self.parse_where_clause()?;
        let signature_end = where_clause
            .as_ref()
            .map(|clause| clause.span.end)
            .or_else(|| return_ty.as_ref().map(|ty| ty.span.end))
            .unwrap_or(params_close.span.end);
        let (body, end) = match self.peek().kind {
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
            _ if modifiers.method_modifier == Some(MethodModifier::Abstract)
                || context == FunctionContext::Interface =>
            {
                (FunctionBody::None, signature_end)
            }
            _ => (FunctionBody::None, signature_end),
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
            operator: modifiers.operator,
            modifier,
            receiver_ty,
            name,
            type_params,
            params,
            return_ty,
            where_clause,
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
    fn parse_struct(&mut self, annotations: Vec<Annotation>) -> Result<StructDecl, Diagnostic> {
        let keyword = self.expect("`struct`", |k| matches!(k, TokenKind::Struct))?;
        let name = self.expect_ident("struct name")?;
        let type_params = self.parse_type_params()?;
        let (fields, mut end) = if matches!(self.peek().kind, TokenKind::LParen) {
            self.bump();
            let mut fields = Vec::new();
            if !matches!(self.peek().kind, TokenKind::RParen) {
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
            }
            let close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
            (StructRepresentationDecl::Declared(fields), close.span.end)
        } else {
            (StructRepresentationDecl::Omitted, name.span.end)
        };
        let interfaces = interfaces_only(self.parse_supertypes(&mut end)?)?;
        let where_clause = self.parse_where_clause()?;
        if let Some(clause) = &where_clause {
            end = clause.span.end;
        }
        let methods = if matches!(self.peek().kind, TokenKind::LBrace) {
            let (methods, body_end) = self.parse_member_body(FunctionContext::TypeBody)?;
            end = body_end;
            methods
        } else {
            Vec::new()
        };
        Ok(StructDecl {
            annotations,
            name,
            type_params,
            fields,
            interfaces,
            where_clause,
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
        annotations: Vec<Annotation>,
        modifier: ClassModifier,
        modifier_span: Option<Span>,
    ) -> Result<ClassDecl, Diagnostic> {
        let keyword = self.expect("`class`", |k| matches!(k, TokenKind::Class))?;
        let name = self.expect_ident("class name")?;
        let type_params = self.parse_type_params()?;
        let mut end = name.span.end;
        let constructor = if matches!(self.peek().kind, TokenKind::LParen) {
            self.bump(); // `(`
            let mut properties = Vec::new();
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
                    properties.push(ConstructorProp {
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
            ClassConstructorDecl::Declared(properties)
        } else {
            ClassConstructorDecl::Omitted
        };
        let supertypes = self.parse_supertypes(&mut end)?;
        let where_clause = self.parse_where_clause()?;
        if let Some(clause) = &where_clause {
            end = clause.span.end;
        }
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
            annotations,
            modifier,
            name,
            type_params,
            constructor,
            base_class: supertypes.base_class,
            interfaces: supertypes.interfaces,
            where_clause,
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
            let supertype = self.parse_named_type_ref_tail(super_name)?;
            *end = supertype.span.end;
            if matches!(self.peek().kind, TokenKind::LParen) {
                if supertypes.base_class.is_some() || !supertypes.interfaces.is_empty() {
                    return Err(Diagnostic::at(
                        self.peek().span,
                        "constructor arguments are only allowed on the base class (the first supertype)",
                    ));
                }
                let (args, args_end) = self.parse_args()?;
                supertypes.base_class = Some((supertype, args));
                *end = args_end;
            } else {
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
    fn parse_interface(
        &mut self,
        annotations: Vec<Annotation>,
    ) -> Result<InterfaceDecl, Diagnostic> {
        let keyword = self.expect("`interface`", |k| matches!(k, TokenKind::Interface))?;
        let name = self.expect_ident("interface name")?;
        let type_params = self.parse_type_params()?;
        let mut end = name.span.end;
        let parents = interfaces_only(self.parse_supertypes(&mut end)?)?;
        let where_clause = self.parse_where_clause()?;
        let (methods, end) = self.parse_member_body(FunctionContext::Interface)?;
        Ok(InterfaceDecl {
            annotations,
            name,
            type_params,
            parents,
            where_clause,
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
        let annotations = if matches!(self.peek().kind, TokenKind::At) {
            self.parse_annotations()?
        } else {
            Vec::new()
        };
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
            if text == "operator" {
                if modifiers.operator.is_some() {
                    return Err(Diagnostic::at(
                        token.span,
                        "duplicate `operator` modifier on member function",
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
        self.parse_function(annotations, modifiers, context)
    }

    /// `enum <name><T, ...>? (: <interface>, ...)? { <variant>, ...
    /// (<fun>, ...)? }` (spec 4.2). The interface list (spec 4.4.3) sits
    /// after the name / type parameters, before `{`. Variants separate
    /// like statements, with `,` in place of `;`; M6 adds member
    /// functions after the variants.
    fn parse_enum(&mut self, annotations: Vec<Annotation>) -> Result<EnumDecl, Diagnostic> {
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
        let close = loop {
            if matches!(self.peek().kind, TokenKind::RBrace) {
                break self.bump();
            }
            if matches!(self.peek().kind, TokenKind::Eof) {
                return self.unexpected("`}`");
            }
            let start = self.pos;
            let parsed = match &self.peek().kind {
                TokenKind::Fun | TokenKind::Suspend | TokenKind::At => self
                    .parse_member_function(FunctionContext::TypeBody)
                    .map(|method| methods.push(method))
                    .and_then(|()| self.expect_statement_end()),
                TokenKind::Ident(text)
                    if matches!(
                        text.as_str(),
                        "override" | "abstract" | "open" | "final" | "operator"
                    ) =>
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
            annotations,
            name,
            type_params,
            variants,
            interfaces,
            where_clause,
            methods,
            span: Span::new(keyword.span.start, close.span.end),
        })
    }

    /// `<T, out U : Interface, ...>`. Variance and upper-bound legality are
    /// intentionally deferred to HIR; the parser preserves the source form.
    fn parse_type_params(&mut self) -> Result<Vec<TypeParamDecl>, Diagnostic> {
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
            let inline_bound = if matches!(self.peek().kind, TokenKind::Colon) {
                self.bump();
                Some(self.parse_type_bound()?)
            } else {
                None
            };
            let start = if variance == Variance::Invariant {
                name.span.start
            } else {
                variance_token.span.start
            };
            let end = self.tokens[self.pos.saturating_sub(1)]
                .span
                .end
                .max(name.span.end);
            type_params.push(TypeParamDecl {
                span: Span::new(start, end),
                name,
                variance,
                inline_bound,
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

    fn parse_type_bound(&mut self) -> Result<TypeBound, Diagnostic> {
        match &self.peek().kind {
            TokenKind::Ident(text) if text == "value" => {
                self.bump();
                Ok(TypeBound::Kind(TypeParamKindBound::Value))
            }
            TokenKind::Ident(text) if text == "ref" => {
                self.bump();
                Ok(TypeBound::Kind(TypeParamKindBound::Ref))
            }
            _ => self.parse_type_ref().map(TypeBound::Upper),
        }
    }

    fn parse_where_clause(&mut self) -> Result<Option<WhereClause>, Diagnostic> {
        let TokenKind::Ident(keyword) = &self.peek().kind else {
            return Ok(None);
        };
        if keyword != "where" {
            return Ok(None);
        }
        let start = self.bump().span.start;
        let mut constraints = Vec::new();
        loop {
            let parameter = self.expect_ident("type parameter name in `where` constraint")?;
            self.expect("`:`", |kind| matches!(kind, TokenKind::Colon))?;
            let bound = self.parse_type_bound()?;
            let end = self.tokens[self.pos - 1].span.end;
            constraints.push(TypeConstraint {
                span: Span::new(parameter.span.start, end),
                parameter,
                bound,
            });
            if !matches!(self.peek().kind, TokenKind::Comma) {
                break;
            }
            self.bump();
        }
        let end = constraints
            .last()
            .expect("a where clause always has one constraint")
            .span
            .end;
        Ok(Some(WhereClause {
            constraints,
            span: Span::new(start, end),
        }))
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
