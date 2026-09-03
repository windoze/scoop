use super::*;

impl Parser {
    /// `struct <name><T, ...>?(val <field>: <type>, ...) (: <interface>, ...)?
    /// ({ <fun>, ... })?` — the type parameter list sits between the name
    /// and the constructor `(`, like `enum` (a `<` right after the name is
    /// unambiguous here). M6 adds an optional interface list (spec 4.4.3)
    /// and an optional member body holding member functions (value
    /// receiver).
    pub(super) fn parse_struct(
        &mut self,
        annotations: Vec<Annotation>,
    ) -> Result<StructDecl, Diagnostic> {
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
    pub(super) fn parse_class(
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
    pub(super) fn parse_supertypes(&mut self, end: &mut u32) -> Result<Supertypes, Diagnostic> {
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
    pub(super) fn parse_interface(
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
    pub(super) fn parse_member_body(
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
    pub(super) fn parse_member_function(
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
}
