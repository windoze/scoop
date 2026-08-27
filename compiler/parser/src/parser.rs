//! Recursive-descent parser for the M6 subset: token vector -> AST.
//!
//! Parsing is fail-fast (see `docs/milestone2/DESIGN.md` section 5): the
//! first error aborts parsing with a single spanned diagnostic. Constructs
//! that are lexically recognizable but outside the subset (`for`, `when`
//! expressions, string interpolation, field assignment, ranges, slices,
//! `super`, secondary constructors, `init` blocks, member properties,
//! companion/`object` declarations, `sealed` classes) get dedicated
//! "not supported" diagnostics rather than generic syntax errors.
//! Declaration parsing lives in `decl.rs`, expression parsing in
//! `expr.rs`, and pattern parsing in `pattern.rs`.

use scoop_ast::{
    Assign, AssignTarget, Block, Diagnostic, Expr, FieldSelector, Ident, If, SourceFile, Span,
    Statement, StatementKind, TypeRef, TypeRefKind, ValDecl, When, WhenArm, While,
};

use crate::lexer::{Token, TokenKind, lex};
use crate::pattern::pattern_span;

pub(crate) fn parse_file(source: &str) -> Result<SourceFile, Diagnostic> {
    let tokens = lex(source)?;
    let mut parser = Parser { tokens, pos: 0 };
    let mut declarations = Vec::new();
    while !parser.at_eof() {
        declarations.push(parser.parse_decl()?);
    }
    Ok(SourceFile {
        declarations,
        span: Span::new(0, source.len() as u32),
    })
}

pub(crate) struct Parser {
    pub(crate) tokens: Vec<Token>,
    pub(crate) pos: usize,
}

impl Parser {
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

    fn at_eof(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
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

    /// A type annotation: a named type, `Unit` (also written `()`), or a
    /// tuple type `(T1, T2, ...)`, each with any number of `?` suffixes
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
            TokenKind::Ident(text) => {
                self.pos += 1;
                if text == "Unit" {
                    Ok(TypeRef {
                        kind: TypeRefKind::Unit,
                        span: token.span,
                    })
                } else if matches!(self.peek().kind, TokenKind::Less) {
                    // Generic type application `Name<T1, T2>`: `<`
                    // directly after a type name is unambiguous in
                    // type position.
                    let start = token.span.start;
                    self.bump();
                    let mut args = vec![self.parse_type_ref()?];
                    while matches!(self.peek().kind, TokenKind::Comma) {
                        self.bump();
                        args.push(self.parse_type_ref()?);
                    }
                    let close = self.expect("`>`", |k| matches!(k, TokenKind::Greater))?;
                    Ok(TypeRef {
                        kind: TypeRefKind::Generic(
                            Ident {
                                text,
                                span: token.span,
                            },
                            args,
                        ),
                        span: Span::new(start, close.span.end),
                    })
                } else {
                    Ok(TypeRef {
                        kind: TypeRefKind::Named(Ident {
                            text,
                            span: token.span,
                        }),
                        span: token.span,
                    })
                }
            }
            TokenKind::LParen => {
                self.pos += 1;
                if matches!(self.peek().kind, TokenKind::RParen) {
                    let close = self.bump();
                    return Ok(TypeRef {
                        kind: TypeRefKind::Unit,
                        span: Span::new(token.span.start, close.span.end),
                    });
                }
                let first = self.parse_type_ref()?;
                if !matches!(self.peek().kind, TokenKind::Comma) {
                    let close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
                    return Ok(TypeRef {
                        kind: first.kind,
                        span: Span::new(token.span.start, close.span.end),
                    });
                }
                let mut elements = vec![first];
                while matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                    if matches!(self.peek().kind, TokenKind::RParen) {
                        break; // trailing comma: `(T,)` / `(T1, T2,)`
                    }
                    elements.push(self.parse_type_ref()?);
                }
                let close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
                Ok(TypeRef {
                    kind: TypeRefKind::Tuple(elements),
                    span: Span::new(token.span.start, close.span.end),
                })
            }
            _ => self.unexpected("type"),
        }
    }

    pub(crate) fn parse_block(&mut self) -> Result<Block, Diagnostic> {
        let open = self.expect("`{`", |k| matches!(k, TokenKind::LBrace))?;
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
                    statements.push(self.parse_statement()?);
                    self.expect_statement_end()?;
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
        Ok(Statement {
            span,
            kind: StatementKind::If(If {
                cond,
                then_block,
                else_block,
                span,
            }),
        })
    }

    /// `when (<subject>) { <arm>* (else -> <block>)? }` — statement-level
    /// pattern matching (spec chapter 5). An arm is
    /// `<pattern> (if (<guard>))? -> <body>` where the body is a block or
    /// a single expression statement (`Red -> println("red")`, spec 5.1);
    /// arms end like statements, and `else` must be the last one.
    fn parse_when(&mut self) -> Result<Statement, Diagnostic> {
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
        Ok(Statement {
            span,
            kind: StatementKind::When(When {
                subject,
                arms,
                else_body,
                span,
            }),
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
}
