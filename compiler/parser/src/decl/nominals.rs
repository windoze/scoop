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
            let mut vararg_span = None;
            if !matches!(self.peek().kind, TokenKind::RParen) {
                loop {
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
                    fields.push(FieldDecl {
                        span: Span::new(
                            modifier_span.map_or(val.span.start, |span| span.start),
                            end,
                        ),
                        name: field_name,
                        ty,
                        syntax,
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
        Ok(StructDecl {
            annotations,
            name,
            type_params,
            fields,
            supertypes,
            where_clause,
            members,
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
            let mut parameters = Vec::new();
            let mut vararg_span = None;
            if !matches!(self.peek().kind, TokenKind::RParen) {
                loop {
                    let modifier_span = self.parse_vararg_modifier(&mut vararg_span)?;
                    let first = self.peek().clone();
                    let property = match first.kind {
                        TokenKind::Val => {
                            self.pos += 1;
                            PrimaryParameterProperty::Val
                        }
                        TokenKind::Var => {
                            self.pos += 1;
                            PrimaryParameterProperty::Var
                        }
                        _ => PrimaryParameterProperty::Plain,
                    };
                    let parameter_name = self.expect_ident("primary constructor parameter name")?;
                    self.expect("`:`", |k| matches!(k, TokenKind::Colon))?;
                    let ty = self.parse_type_ref()?;
                    let (syntax, end) = self.parse_parameter_syntax(modifier_span, ty.span.end)?;
                    parameters.push(PrimaryClassParameter {
                        property,
                        span: Span::new(
                            modifier_span.map_or(first.span.start, |span| span.start),
                            end,
                        ),
                        name: parameter_name,
                        ty,
                        syntax,
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
            ClassConstructorDecl::Declared(parameters)
        } else {
            ClassConstructorDecl::Omitted
        };
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
            .unwrap_or(keyword.span.start);
        Ok(ClassDecl {
            annotations,
            modifier,
            name,
            type_params,
            constructor,
            supertypes,
            where_clause,
            members,
            span: Span::new(start, end),
        })
    }

    pub(super) fn parse_vararg_modifier(
        &mut self,
        existing: &mut Option<Span>,
    ) -> Result<Option<Span>, Diagnostic> {
        if !matches!(self.peek().kind, TokenKind::Vararg) {
            return Ok(None);
        }
        let modifier = self.bump();
        if matches!(self.peek().kind, TokenKind::Vararg) {
            return Err(Diagnostic::at(
                self.peek().span,
                "duplicate `vararg` modifier on parameter",
            ));
        }
        if existing.is_some() {
            return Err(Diagnostic::at(
                modifier.span,
                "a parameter list may declare at most one `vararg` parameter",
            ));
        }
        *existing = Some(modifier.span);
        Ok(Some(modifier.span))
    }

    /// `: Base(args), I1, I2` — a no-op when the next token is not `:`.
    /// Every entry retains whether its argument list was present. HIR, which
    /// knows nominal kinds, decides which entry is the direct base.
    pub(super) fn parse_supertypes(
        &mut self,
        end: &mut u32,
    ) -> Result<Vec<SupertypeSpec>, Diagnostic> {
        let mut supertypes = Vec::new();
        if !matches!(self.peek().kind, TokenKind::Colon) {
            return Ok(supertypes);
        }
        self.bump(); // `:`
        loop {
            let super_name = self.expect_ident("base class or interface name")?;
            let supertype = self.parse_named_type_ref_tail(super_name)?;
            let start = supertype.span.start;
            *end = supertype.span.end;
            let constructor_arguments = if matches!(self.peek().kind, TokenKind::LParen) {
                let (args, args_end) = self.parse_args()?;
                *end = args_end;
                Some(args)
            } else {
                None
            };
            supertypes.push(SupertypeSpec {
                ty: supertype,
                constructor_arguments,
                span: Span::new(start, *end),
            });
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
        let supertypes = self.parse_supertypes(&mut end)?;
        let where_clause = self.parse_where_clause()?;
        let (methods, end) = self.parse_member_body(FunctionContext::Interface)?;
        Ok(InterfaceDecl {
            annotations,
            name,
            type_params,
            supertypes,
            where_clause,
            methods,
            span: Span::new(keyword.span.start, end),
        })
    }

    fn parse_class_body(&mut self) -> Result<(Vec<ClassMember>, u32), Diagnostic> {
        self.bump(); // `{`
        let body_depth = self.brace_depth();
        let mut members = Vec::new();
        let close = loop {
            if matches!(self.peek().kind, TokenKind::RBrace) {
                break self.bump();
            }
            if matches!(self.peek().kind, TokenKind::Eof) {
                return self.unexpected("`}`");
            }
            let start = self.pos;
            let parsed = self.parse_class_member();
            match parsed {
                Ok(member) => {
                    members.push(member);
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
        Ok((members, close.span.end))
    }

    fn parse_class_member(&mut self) -> Result<ClassMember, Diagnostic> {
        match &self.peek().kind {
            TokenKind::Val | TokenKind::Var => self
                .parse_stored_property()
                .map(ClassMember::StoredProperty),
            TokenKind::Ident(text) if text == "init" => {
                self.parse_init_block().map(ClassMember::InitBlock)
            }
            TokenKind::Ident(text) if text == "constructor" => self
                .parse_secondary_constructor()
                .map(ClassMember::SecondaryConstructor),
            TokenKind::Struct | TokenKind::Enum | TokenKind::Class | TokenKind::Interface => {
                Err(Diagnostic::at(
                    self.peek().span,
                    "nested type declarations are not supported yet (milestone M21)",
                ))
            }
            TokenKind::Ident(text) if text == "companion" => Err(Diagnostic::at(
                self.peek().span,
                "companion objects are not supported yet (milestone M21)",
            )),
            TokenKind::Ident(text) if text == "object" => Err(Diagnostic::at(
                self.peek().span,
                "`object` declarations are not supported yet (milestone M21)",
            )),
            _ => self
                .parse_member_function(FunctionContext::TypeBody)
                .map(ClassMember::Function),
        }
    }

    fn parse_struct_body(&mut self) -> Result<(Vec<StructMember>, u32), Diagnostic> {
        self.bump(); // `{`
        let body_depth = self.brace_depth();
        let mut members = Vec::new();
        let close = loop {
            if matches!(self.peek().kind, TokenKind::RBrace) {
                break self.bump();
            }
            if matches!(self.peek().kind, TokenKind::Eof) {
                return self.unexpected("`}`");
            }
            let start = self.pos;
            let parsed = match &self.peek().kind {
                TokenKind::Ident(text) if text == "constructor" => self
                    .parse_secondary_constructor()
                    .map(StructMember::SecondaryConstructor),
                TokenKind::Val | TokenKind::Var => Err(Diagnostic::at(
                    self.peek().span,
                    "struct body stored properties are not allowed",
                )),
                TokenKind::Ident(text) if text == "init" => Err(Diagnostic::at(
                    self.peek().span,
                    "`init` blocks are not allowed in structs",
                )),
                TokenKind::Struct | TokenKind::Enum | TokenKind::Class | TokenKind::Interface => {
                    Err(Diagnostic::at(
                        self.peek().span,
                        "nested type declarations are not supported yet (milestone M21)",
                    ))
                }
                TokenKind::Ident(text) if text == "companion" => Err(Diagnostic::at(
                    self.peek().span,
                    "companion objects are not supported yet (milestone M21)",
                )),
                TokenKind::Ident(text) if text == "object" => Err(Diagnostic::at(
                    self.peek().span,
                    "`object` declarations are not supported yet (milestone M21)",
                )),
                _ => self
                    .parse_member_function(FunctionContext::TypeBody)
                    .map(Box::new)
                    .map(StructMember::Function),
            };
            match parsed {
                Ok(member) => {
                    members.push(member);
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
        Ok((members, close.span.end))
    }

    fn parse_stored_property(&mut self) -> Result<StoredPropertyDecl, Diagnostic> {
        let keyword = self.bump();
        let mutable = matches!(keyword.kind, TokenKind::Var);
        let name = self.expect_ident("stored property name")?;
        if !matches!(self.peek().kind, TokenKind::Colon) {
            return Err(Diagnostic::at(
                self.peek().span,
                "class stored properties require an explicit type",
            ));
        }
        self.bump();
        let ty = self.parse_type_ref()?;
        if !matches!(self.peek().kind, TokenKind::Equal) {
            return Err(Diagnostic::at(
                self.peek().span,
                "class stored properties require an initializer",
            ));
        }
        self.bump();
        let initializer = self.parse_expr()?;
        Ok(StoredPropertyDecl {
            mutable,
            name,
            ty,
            span: Span::new(keyword.span.start, initializer.span().end),
            initializer,
        })
    }

    fn parse_init_block(&mut self) -> Result<InitBlockDecl, Diagnostic> {
        let keyword = self.bump();
        if !matches!(self.peek().kind, TokenKind::LBrace) {
            return Err(Diagnostic::at(
                self.peek().span,
                "`init` must be followed by a block",
            ));
        }
        let body = self.parse_block()?;
        Ok(InitBlockDecl {
            span: Span::new(keyword.span.start, body.span.end),
            body,
        })
    }

    fn parse_secondary_constructor(&mut self) -> Result<SecondaryConstructorDecl, Diagnostic> {
        let keyword = self.bump();
        if matches!(self.peek().kind, TokenKind::Less) {
            return Err(Diagnostic::at(
                self.peek().span,
                "secondary constructors cannot declare type parameters",
            ));
        }
        self.expect("`(` after `constructor`", |kind| {
            matches!(kind, TokenKind::LParen)
        })?;
        let mut params = Vec::new();
        let mut vararg_span = None;
        if !matches!(self.peek().kind, TokenKind::RParen) {
            loop {
                let modifier_span = self.parse_vararg_modifier(&mut vararg_span)?;
                if matches!(self.peek().kind, TokenKind::Val | TokenKind::Var) {
                    return Err(Diagnostic::at(
                        self.peek().span,
                        "secondary constructor parameters cannot declare `val` or `var` properties",
                    ));
                }
                let name = self.expect_ident("constructor parameter name")?;
                self.expect("`:`", |kind| matches!(kind, TokenKind::Colon))?;
                let ty = self.parse_type_ref()?;
                let (syntax, end) = self.parse_parameter_syntax(modifier_span, ty.span.end)?;
                params.push(Param {
                    span: Span::new(
                        modifier_span.map_or(name.span.start, |span| span.start),
                        end,
                    ),
                    name,
                    ty,
                    syntax,
                });
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        self.expect("`)`", |kind| matches!(kind, TokenKind::RParen))?;
        let delegation = if matches!(self.peek().kind, TokenKind::Colon) {
            self.bump();
            Some(self.parse_constructor_delegation()?)
        } else {
            None
        };
        if !matches!(self.peek().kind, TokenKind::LBrace) {
            return Err(Diagnostic::at(
                self.peek().span,
                "secondary constructors require a block body",
            ));
        }
        let body = self.parse_block()?;
        Ok(SecondaryConstructorDecl {
            params,
            delegation,
            span: Span::new(keyword.span.start, body.span.end),
            body,
        })
    }

    fn parse_constructor_delegation(&mut self) -> Result<ConstructorDelegation, Diagnostic> {
        let target = self.peek().clone();
        let is_this = match &target.kind {
            TokenKind::This => true,
            TokenKind::Ident(text) if text == "super" => false,
            _ => {
                return Err(Diagnostic::at(
                    target.span,
                    "constructor delegation target must be `this` or `super`",
                ));
            }
        };
        self.bump();
        if !matches!(self.peek().kind, TokenKind::LParen) {
            return Err(Diagnostic::at(
                self.peek().span,
                "constructor delegation requires an argument list",
            ));
        }
        let (arguments, end) = self.parse_args()?;
        let span = Span::new(target.span.start, end);
        Ok(if is_this {
            ConstructorDelegation::This {
                target_span: target.span,
                arguments,
                span,
            }
        } else {
            ConstructorDelegation::Super {
                target_span: target.span,
                arguments,
                span,
            }
        })
    }

    /// Function-only type body used by interfaces. Classes and structs have
    /// their own ordered member parsers above.
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
            if matches!(token.kind, TokenKind::Infix) {
                if modifiers.infix.is_some() {
                    return Err(Diagnostic::at(
                        token.span,
                        "duplicate `infix` modifier on member function",
                    ));
                }
                let keyword = self.bump();
                modifiers.infix = Some(InfixModifier { span: keyword.span });
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
