//! Recursive-descent parser for the M10 subset: token vector -> AST.
//!
//! Parsing recovers at declaration, member and statement boundaries and
//! returns every independent spanned diagnostic found in one file. A file
//! with any diagnostic does not produce an AST. Constructs that are
//! lexically recognizable but outside the subset (`for`, `when`
//! expressions, string interpolation, field assignment, ranges, slices,
//! `super`, secondary constructors, `init` blocks, member properties,
//! companion/`object` declarations, `sealed` classes, `try` expressions)
//! get dedicated "not supported" diagnostics rather than generic syntax
//! errors. Declaration parsing lives in `decl.rs`, expression parsing in
//! `expr.rs`, and pattern parsing in `pattern.rs`.

use scoop_ast::{
    Assign, AssignTarget, Block, CatchClause, Diagnostic, Expr, FieldSelector, Ident, If,
    SourceFile, Span, Statement, StatementKind, Try, TypeRef, TypeRefKind, ValDecl, When, WhenArm,
    While,
};

use crate::lexer::{Token, TokenKind, lex};
use crate::pattern::pattern_span;

pub(crate) fn parse_file(source: &str) -> Result<SourceFile, Vec<Diagnostic>> {
    let (tokens, lexical_diagnostics) = lex(source);
    if !lexical_diagnostics.is_empty() {
        return Err(lexical_diagnostics);
    }
    let mut parser = Parser {
        tokens,
        pos: 0,
        diagnostics: Vec::new(),
        next_lambda_id: 0,
        next_anonymous_function_id: 0,
        next_callable_reference_id: 0,
    };
    let mut declarations = Vec::new();
    while !parser.at_eof() {
        let start = parser.pos;
        match parser.parse_decl() {
            Ok(decl) => declarations.push(decl),
            Err(diagnostic) => {
                parser.diagnostics.push(diagnostic);
                parser.synchronize_top_level(start);
            }
        }
    }
    let file = SourceFile {
        declarations,
        span: Span::new(0, source.len() as u32),
    };
    if parser.diagnostics.is_empty() {
        Ok(file)
    } else {
        Err(parser.diagnostics)
    }
}

pub(crate) struct Parser {
    pub(crate) tokens: Vec<Token>,
    pub(crate) pos: usize,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) next_lambda_id: u32,
    pub(crate) next_anonymous_function_id: u32,
    pub(crate) next_callable_reference_id: u32,
}

impl Parser {
    pub(crate) fn alloc_lambda_id(&mut self) -> scoop_ast::LambdaId {
        let id = scoop_ast::LambdaId(self.next_lambda_id);
        self.next_lambda_id += 1;
        id
    }

    pub(crate) fn alloc_anonymous_function_id(&mut self) -> scoop_ast::AnonymousFunctionId {
        let id = scoop_ast::AnonymousFunctionId(self.next_anonymous_function_id);
        self.next_anonymous_function_id += 1;
        id
    }

    pub(crate) fn alloc_callable_reference_id(&mut self) -> scoop_ast::CallableReferenceId {
        let id = scoop_ast::CallableReferenceId(self.next_callable_reference_id);
        self.next_callable_reference_id += 1;
        id
    }

    pub(crate) fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    pub(crate) fn bump(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();
        if !matches!(token.kind, TokenKind::Eof) {
            self.pos += 1;
        }
        token
    }

    pub(crate) fn at_eof(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    /// Number of unmatched `{` tokens before the current position.
    /// Computing it on demand keeps direct token advances in the small
    /// parsing routines from having to maintain duplicate delimiter state.
    pub(crate) fn brace_depth(&self) -> usize {
        self.tokens[..self.pos]
            .iter()
            .fold(0usize, |depth, token| match token.kind {
                TokenKind::LBrace => depth + 1,
                TokenKind::RBrace => depth.saturating_sub(1),
                _ => depth,
            })
    }

    fn is_top_level_start(&self) -> bool {
        match &self.peek().kind {
            TokenKind::Suspend
            | TokenKind::Fun
            | TokenKind::Struct
            | TokenKind::Enum
            | TokenKind::Class
            | TokenKind::Interface
            | TokenKind::At => true,
            TokenKind::Ident(text) => {
                matches!(text.as_str(), "open" | "abstract" | "sealed" | "object")
            }
            _ => false,
        }
    }

    /// Skip a malformed declaration without consuming the next declaration
    /// starter at brace depth zero. Delimiters inside the malformed item are
    /// crossed before synchronization, so errors in a body do not turn each
    /// remaining statement into a top-level diagnostic.
    fn synchronize_top_level(&mut self, item_start: usize) {
        while !self.at_eof() {
            if self.pos > item_start
                && self.brace_depth() == 0
                && self.peek().newline_before
                && self.is_top_level_start()
            {
                return;
            }
            self.bump();
        }
    }

    /// Recover to the next line/semicolon at `target_depth`, or stop before
    /// the brace closing the current body. `item_start` prevents a token that
    /// itself failed at the beginning of an item from being retried forever.
    pub(crate) fn synchronize_body_item(&mut self, target_depth: usize, item_start: usize) {
        while !self.at_eof() {
            let depth = self.brace_depth();
            if depth < target_depth
                || (depth == target_depth && matches!(self.peek().kind, TokenKind::RBrace))
            {
                return;
            }
            if self.pos > item_start && depth == target_depth && self.peek().newline_before {
                return;
            }
            let token = self.bump();
            if depth == target_depth && matches!(token.kind, TokenKind::Semicolon) {
                return;
            }
        }
    }

    /// Builds "expected <what>, found <token>" at the current token.
    pub(crate) fn unexpected<T>(&self, what: &str) -> Result<T, Diagnostic> {
        let token = self.peek();
        Err(Diagnostic::at(
            token.span,
            format!("expected {what}, found {}", token.describe()),
        ))
    }

    pub(crate) fn expect(
        &mut self,
        what: &str,
        pred: impl Fn(&TokenKind) -> bool,
    ) -> Result<Token, Diagnostic> {
        if pred(&self.peek().kind) {
            Ok(self.bump())
        } else {
            self.unexpected(what)
        }
    }

    pub(crate) fn expect_ident(&mut self, what: &str) -> Result<Ident, Diagnostic> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Ident(text) => {
                self.pos += 1;
                Ok(Ident {
                    text,
                    span: token.span,
                })
            }
            _ => self.unexpected(what),
        }
    }

    /// A type annotation: a named type, `Unit` (also written `()`), a
    /// tuple type `(T1, T2, ...)`, or a function type `(P...) -> R` /
    /// `suspend (P...) -> R`, each with any number of `?` suffixes
    /// (`T?` is `Option<T>`, and `T??` does not collapse — spec 7.1).
    /// Parenthesized disambiguation mirrors expressions (spec section
    /// 4.3): `(T)` is just `T` in parentheses, `(T,)` a 1-tuple type.
    pub(crate) fn parse_type_ref(&mut self) -> Result<TypeRef, Diagnostic> {
        let mut ty = self.parse_type_atom()?;
        while matches!(self.peek().kind, TokenKind::Question) {
            let question = self.bump();
            ty = TypeRef {
                span: Span::new(ty.span.start, question.span.end),
                kind: TypeRefKind::Nullable(Box::new(ty)),
            };
        }
        Ok(ty)
    }

    fn parse_type_atom(&mut self) -> Result<TypeRef, Diagnostic> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Suspend => {
                self.pos += 1;
                self.parse_paren_type(true, token.span.start)
            }
            TokenKind::Ident(text) => {
                self.pos += 1;
                self.parse_named_type_ref_tail(Ident {
                    text,
                    span: token.span,
                })
            }
            TokenKind::LParen => self.parse_paren_type(false, token.span.start),
            _ => self.unexpected("type"),
        }
    }

    /// Parse a parenthesized/tuple type or a function type parameter list.
    /// `suspend` has already been consumed when `is_suspend` is true.
    fn parse_paren_type(&mut self, is_suspend: bool, start: u32) -> Result<TypeRef, Diagnostic> {
        self.expect("`(`", |k| matches!(k, TokenKind::LParen))?;
        let mut elements = Vec::new();
        let mut had_comma = false;
        if !matches!(self.peek().kind, TokenKind::RParen) {
            elements.push(self.parse_type_ref()?);
            while matches!(self.peek().kind, TokenKind::Comma) {
                had_comma = true;
                self.bump();
                if matches!(self.peek().kind, TokenKind::RParen) {
                    break;
                }
                elements.push(self.parse_type_ref()?);
            }
        }
        let close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;

        if matches!(self.peek().kind, TokenKind::Arrow) {
            self.bump();
            let return_type = self.parse_type_ref()?;
            return Ok(TypeRef {
                span: Span::new(start, return_type.span.end),
                kind: TypeRefKind::Function(scoop_ast::FunctionTypeRef {
                    is_suspend,
                    parameters: elements,
                    return_type: Box::new(return_type),
                }),
            });
        }

        if is_suspend {
            return Err(Diagnostic::at(
                self.peek().span,
                format!(
                    "expected `->` in suspend function type, found {}",
                    self.peek().describe()
                ),
            ));
        }
        if elements.is_empty() {
            return Ok(TypeRef {
                kind: TypeRefKind::Unit,
                span: Span::new(start, close.span.end),
            });
        }
        if elements.len() == 1 && !had_comma {
            let first = elements.pop().expect("one parenthesized type");
            return Ok(TypeRef {
                kind: first.kind,
                span: Span::new(start, close.span.end),
            });
        }
        Ok(TypeRef {
            kind: TypeRefKind::Tuple(elements),
            span: Span::new(start, close.span.end),
        })
    }

    /// Complete a named type after its identifier was consumed. Supertype
    /// lists use the same generic-application grammar as annotations.
    pub(crate) fn parse_named_type_ref_tail(&mut self, name: Ident) -> Result<TypeRef, Diagnostic> {
        if name.text == "Unit" {
            Ok(TypeRef {
                kind: TypeRefKind::Unit,
                span: name.span,
            })
        } else if matches!(self.peek().kind, TokenKind::Less) {
            let start = name.span.start;
            self.bump();
            let mut args = vec![self.parse_type_ref()?];
            while matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
                args.push(self.parse_type_ref()?);
            }
            let close = self.expect("`>`", |k| matches!(k, TokenKind::Greater))?;
            Ok(TypeRef {
                kind: TypeRefKind::Generic(name, args),
                span: Span::new(start, close.span.end),
            })
        } else {
            Ok(TypeRef {
                span: name.span,
                kind: TypeRefKind::Named(name),
            })
        }
    }

    pub(crate) fn parse_block(&mut self) -> Result<Block, Diagnostic> {
        let open = self.expect("`{`", |k| matches!(k, TokenKind::LBrace))?;
        self.parse_block_after_open(open)
    }

    /// Parse block items after the opening brace has already been consumed.
    /// Lambda parsing uses this after its optional parameter header.
    pub(crate) fn parse_block_after_open(&mut self, open: Token) -> Result<Block, Diagnostic> {
        let body_depth = self.brace_depth();
        let mut statements = Vec::new();
        loop {
            match self.peek().kind {
                TokenKind::RBrace => {
                    let close = self.bump();
                    return Ok(Block {
                        statements,
                        span: Span::new(open.span.start, close.span.end),
                    });
                }
                TokenKind::Eof => return self.unexpected("`}`"),
                _ => {
                    let start = self.pos;
                    match self.parse_statement() {
                        Ok(statement) => {
                            statements.push(statement);
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
                }
            }
        }
    }

    /// A statement ends at a newline, at `}`, or at an explicit `;`.
    pub(crate) fn expect_statement_end(&mut self) -> Result<(), Diagnostic> {
        if matches!(self.peek().kind, TokenKind::Semicolon) {
            while matches!(self.peek().kind, TokenKind::Semicolon) {
                self.bump();
            }
            return Ok(());
        }
        let token = self.peek();
        if token.newline_before || matches!(token.kind, TokenKind::RBrace | TokenKind::Eof) {
            Ok(())
        } else {
            self.unexpected("`;` or newline after statement")
        }
    }

    fn parse_statement(&mut self) -> Result<Statement, Diagnostic> {
        match &self.peek().kind {
            TokenKind::Fun if !matches!(self.tokens[self.pos + 1].kind, TokenKind::LParen) => {
                let function = self.parse_function(
                    Vec::new(),
                    crate::decl::Modifiers::default(),
                    crate::decl::FunctionContext::Local,
                )?;
                Ok(Statement {
                    span: function.span,
                    kind: StatementKind::LocalFunction(function),
                })
            }
            TokenKind::Suspend
                if matches!(self.tokens[self.pos + 1].kind, TokenKind::Fun)
                    && !matches!(self.tokens[self.pos + 2].kind, TokenKind::LParen) =>
            {
                let suspend = self.bump();
                let function = self.parse_function(
                    Vec::new(),
                    crate::decl::Modifiers {
                        is_suspend: true,
                        suspend_span: Some(suspend.span),
                        start: Some(suspend.span.start),
                        ..crate::decl::Modifiers::default()
                    },
                    crate::decl::FunctionContext::Local,
                )?;
                Ok(Statement {
                    span: function.span,
                    kind: StatementKind::LocalFunction(function),
                })
            }
            TokenKind::Val | TokenKind::Var => self.parse_val_decl(),
            TokenKind::If => self.parse_if(),
            TokenKind::When => self.parse_when(),
            TokenKind::While => self.parse_while(),
            TokenKind::Return => self.parse_return(),
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                Ok(Statement {
                    span: block.span,
                    kind: StatementKind::Block(block),
                })
            }
            TokenKind::At => Err(Diagnostic::at(
                self.peek().span,
                "annotations are only allowed on function declarations (milestone M4)",
            )),
            TokenKind::Ident(text) if text == "for" => Err(Diagnostic::at(
                self.peek().span,
                "`for` loops are not supported yet (milestone M5)",
            )),
            // `try` / `catch` / `finally` / `throw` are contextual: they
            // stay identifiers everywhere except statement position.
            TokenKind::Ident(text) if text == "try" => self.parse_try(),
            TokenKind::Ident(text) if text == "throw" => self.parse_throw(),
            // `name = ...` (a single `=`, not `==`) assigns to a local `var`.
            TokenKind::Ident(_) if matches!(self.tokens[self.pos + 1].kind, TokenKind::Equal) => {
                self.parse_assign()
            }
            _ => {
                let expr = self.parse_expr()?;
                if matches!(self.peek().kind, TokenKind::Equal) {
                    match expr {
                        // `m[i] = v` — subscript assignment (M5).
                        Expr::Index {
                            receiver,
                            index,
                            span,
                        } => {
                            self.bump(); // `=`
                            let value = self.parse_expr()?;
                            let assign_span = Span::new(span.start, value.span().end);
                            return Ok(Statement {
                                span: assign_span,
                                kind: StatementKind::Assign(Assign {
                                    target: AssignTarget::Index {
                                        receiver,
                                        index,
                                        span,
                                    },
                                    value,
                                    span: assign_span,
                                }),
                            });
                        }
                        // `obj.field = v` — a field assignment (M6). The
                        // parser cannot tell class `var` properties from
                        // value-type fields; HIR rejects the latter.
                        Expr::FieldAccess(access) => {
                            if access.safe {
                                return Err(Diagnostic::at(
                                    access.span,
                                    "assignments through `?.` are not allowed",
                                ));
                            }
                            let FieldSelector::Name(name) = access.selector else {
                                // Tuple elements are value-type fields.
                                return Err(Diagnostic::at(
                                    self.peek().span,
                                    "field assignment is not supported (value types are immutable)",
                                ));
                            };
                            self.bump(); // `=`
                            let value = self.parse_expr()?;
                            let assign_span = Span::new(access.span.start, value.span().end);
                            return Ok(Statement {
                                span: assign_span,
                                kind: StatementKind::Assign(Assign {
                                    target: AssignTarget::Field {
                                        receiver: access.receiver,
                                        name,
                                        span: access.span,
                                    },
                                    value,
                                    span: assign_span,
                                }),
                            });
                        }
                        _ => return self.unexpected("`;` or newline after statement"),
                    }
                }
                Ok(Statement {
                    span: expr.span(),
                    kind: StatementKind::Expr(expr),
                })
            }
        }
    }

    /// `(val|var) <pattern> (: <type>)? = <expr>` — the initializer is
    /// mandatory. A plain identifier target is `Pattern::Binding`;
    /// destructuring uses tuple/struct patterns (spec 4.6). `var` with a
    /// pattern behaves like `val` (value types are immutable).
    fn parse_val_decl(&mut self) -> Result<Statement, Diagnostic> {
        let keyword = self.bump();
        let mutable = matches!(keyword.kind, TokenKind::Var);
        let target = self.parse_pattern()?;
        let ty = if matches!(self.peek().kind, TokenKind::Colon) {
            self.bump();
            Some(self.parse_type_ref()?)
        } else {
            None
        };
        self.expect("`=`", |k| matches!(k, TokenKind::Equal))?;
        let init = self.parse_expr()?;
        let span = Span::new(keyword.span.start, init.span().end);
        Ok(Statement {
            span,
            kind: StatementKind::ValDecl(ValDecl {
                mutable,
                target,
                ty,
                init,
                span,
            }),
        })
    }

    /// `<name> = <expr>` — assigns to a plain local `var`. Subscript
    /// assignment (`m[i] = v`) is handled in `parse_statement`, where the
    /// target is a full expression.
    fn parse_assign(&mut self) -> Result<Statement, Diagnostic> {
        let target = self.expect_ident("assignment target")?;
        self.bump(); // `=`
        let value = self.parse_expr()?;
        let span = Span::new(target.span.start, value.span().end);
        Ok(Statement {
            span,
            kind: StatementKind::Assign(Assign {
                target: AssignTarget::Local(target),
                value,
                span,
            }),
        })
    }

    /// `if (<cond>) { ... } (else { ... })?` — the condition is always
    /// parenthesized. `else if` chains get no special treatment (M2 design
    /// section 6): `else` must be followed by a block.
    fn parse_if(&mut self) -> Result<Statement, Diagnostic> {
        let if_ = self.parse_if_node()?;
        Ok(Statement {
            span: if_.span,
            kind: StatementKind::If(if_),
        })
    }

    pub(crate) fn parse_if_expression(&mut self) -> Result<Expr, Diagnostic> {
        Ok(Expr::If(Box::new(self.parse_if_node()?)))
    }

    fn parse_if_node(&mut self) -> Result<If, Diagnostic> {
        let keyword = self.bump(); // `if`
        self.expect("`(`", |k| matches!(k, TokenKind::LParen))?;
        let cond = self.parse_expr()?;
        self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
        let then_block = self.parse_block()?;
        let mut end = then_block.span.end;
        let else_block = if matches!(self.peek().kind, TokenKind::Else) {
            self.bump();
            let block = self.parse_block()?;
            end = block.span.end;
            Some(block)
        } else {
            None
        };
        let span = Span::new(keyword.span.start, end);
        Ok(If {
            cond,
            then_block,
            else_block,
            span,
        })
    }

    /// `when (<subject>) { <arm>* (else -> <block>)? }` — statement-level
    /// pattern matching (spec chapter 5). An arm is
    /// `<pattern> (if (<guard>))? -> <body>` where the body is a block or
    /// a single expression statement (`Red -> println("red")`, spec 5.1);
    /// arms end like statements, and `else` must be the last one.
    fn parse_when(&mut self) -> Result<Statement, Diagnostic> {
        let when = self.parse_when_node()?;
        Ok(Statement {
            span: when.span,
            kind: StatementKind::When(when),
        })
    }

    pub(crate) fn parse_when_expression(&mut self) -> Result<Expr, Diagnostic> {
        Ok(Expr::When(Box::new(self.parse_when_node()?)))
    }

    fn parse_when_node(&mut self) -> Result<When, Diagnostic> {
        let keyword = self.bump(); // `when`
        self.expect("`(`", |k| matches!(k, TokenKind::LParen))?;
        let subject = self.parse_expr()?;
        self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
        self.expect("`{`", |k| matches!(k, TokenKind::LBrace))?;
        let mut arms = Vec::new();
        let mut else_body = None;
        let close = loop {
            match self.peek().kind {
                TokenKind::RBrace => break self.bump(),
                TokenKind::Eof => return self.unexpected("`}`"),
                TokenKind::Else => {
                    self.bump();
                    self.expect("`->`", |k| matches!(k, TokenKind::Arrow))?;
                    else_body = Some(self.parse_arm_body()?);
                    // `else` must be the last arm.
                    break self.expect("`}`", |k| matches!(k, TokenKind::RBrace))?;
                }
                _ => {
                    let pattern = self.parse_pattern()?;
                    let guard = if matches!(self.peek().kind, TokenKind::If) {
                        self.bump();
                        self.expect("`(`", |k| matches!(k, TokenKind::LParen))?;
                        let guard = self.parse_expr()?;
                        self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
                        Some(guard)
                    } else {
                        None
                    };
                    self.expect("`->`", |k| matches!(k, TokenKind::Arrow))?;
                    let body = self.parse_arm_body()?;
                    let span = Span::new(pattern_span(&pattern).start, body.span.end);
                    arms.push(WhenArm {
                        pattern,
                        guard,
                        body,
                        span,
                    });
                    self.expect_statement_end()?;
                }
            }
        };
        let span = Span::new(keyword.span.start, close.span.end);
        Ok(When {
            subject,
            arms,
            else_body,
            span,
        })
    }

    /// A `when` arm body: a block, or a single expression statement
    /// wrapped in a synthetic block (the AST keeps `Block` either way).
    fn parse_arm_body(&mut self) -> Result<Block, Diagnostic> {
        if matches!(self.peek().kind, TokenKind::LBrace) {
            return self.parse_block();
        }
        let expr = self.parse_expr()?;
        let span = expr.span();
        Ok(Block {
            statements: vec![Statement {
                kind: StatementKind::Expr(expr),
                span,
            }],
            span,
        })
    }

    /// `return <expr>?` — a bare `return` (for `Unit` functions) ends at a
    /// newline, `;`, `}`, or end of file; anything else is the return value.
    fn parse_return(&mut self) -> Result<Statement, Diagnostic> {
        let keyword = self.bump(); // `return`
        let token = self.peek();
        let has_value = !token.newline_before
            && !matches!(
                token.kind,
                TokenKind::RBrace | TokenKind::Semicolon | TokenKind::Eof
            );
        let value = if has_value {
            Some(self.parse_expr()?)
        } else {
            None
        };
        let end = value
            .as_ref()
            .map(|expr| expr.span().end)
            .unwrap_or(keyword.span.end);
        Ok(Statement {
            span: Span::new(keyword.span.start, end),
            kind: StatementKind::Return { value },
        })
    }

    /// `while (<cond>) { ... }` — the condition is always parenthesized.
    fn parse_while(&mut self) -> Result<Statement, Diagnostic> {
        let keyword = self.bump(); // `while`
        self.expect("`(`", |k| matches!(k, TokenKind::LParen))?;
        let cond = self.parse_expr()?;
        self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
        let body = self.parse_block()?;
        let span = Span::new(keyword.span.start, body.span.end);
        Ok(Statement {
            span,
            kind: StatementKind::While(While { cond, body, span }),
        })
    }

    /// `try { ... } (catch (<name>: <type>) { ... })* (finally { ... })?`
    /// — statement wrapper around the same payload used by `Expr::Try`.
    /// At least one `catch` or a `finally` is required.
    /// Catch parameters always carry a type annotation (M8 has no
    /// inference for them); `catch` clauses attach to the `try` even
    /// across newlines.
    fn parse_try(&mut self) -> Result<Statement, Diagnostic> {
        let try_ = self.parse_try_node()?;
        Ok(Statement {
            span: try_.span,
            kind: StatementKind::Try(try_),
        })
    }

    pub(crate) fn parse_try_expression(&mut self) -> Result<Expr, Diagnostic> {
        Ok(Expr::Try(Box::new(self.parse_try_node()?)))
    }

    fn parse_try_node(&mut self) -> Result<Try, Diagnostic> {
        let keyword = self.bump(); // `try`
        let body = self.parse_block()?;
        let mut end = body.span.end;
        let mut catches = Vec::new();
        while matches!(&self.peek().kind, TokenKind::Ident(text) if text == "catch") {
            let catch_keyword = self.bump();
            self.expect("`(`", |k| matches!(k, TokenKind::LParen))?;
            let name = self.expect_ident("catch parameter name")?;
            if !matches!(self.peek().kind, TokenKind::Colon) {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "catch parameter requires a type annotation",
                ));
            }
            self.bump(); // `:`
            let ty = self.parse_type_ref()?;
            self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
            let catch_body = self.parse_block()?;
            end = catch_body.span.end;
            catches.push(CatchClause {
                name,
                ty,
                body: catch_body,
                span: Span::new(catch_keyword.span.start, end),
            });
        }
        let finally_body = if matches!(&self.peek().kind, TokenKind::Ident(text) if text == "finally")
        {
            self.bump();
            let block = self.parse_block()?;
            end = block.span.end;
            Some(block)
        } else {
            None
        };
        if catches.is_empty() && finally_body.is_none() {
            return self.unexpected("`catch` or `finally`");
        }
        let span = Span::new(keyword.span.start, end);
        Ok(Try {
            body,
            catches,
            finally_body,
            span,
        })
    }

    /// `throw <expr>` — the operand is a full expression running to the
    /// end of the statement (`throw f(1) + 2` throws `f(1) + 2`).
    fn parse_throw(&mut self) -> Result<Statement, Diagnostic> {
        let keyword = self.bump(); // `throw`
        let value = self.parse_expr()?;
        let span = Span::new(keyword.span.start, value.span().end);
        Ok(Statement {
            span,
            kind: StatementKind::Throw(value),
        })
    }
}
