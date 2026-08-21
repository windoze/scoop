//! Recursive-descent parser for the M2 subset: token vector -> AST.
//!
//! M2 is fail-fast (see `docs/milestone2/DESIGN.md` section 5): the first
//! error aborts parsing with a single spanned diagnostic. Constructs that
//! are lexically recognizable but outside the subset (parameters, return
//! type annotations, `enum`, `when`, `for`, string interpolation, field
//! assignment) get dedicated "not supported" diagnostics rather than
//! generic syntax errors. Expression parsing lives in `expr.rs`.

use scoop_ast::{
    Assign, Block, Decl, Diagnostic, Expr, FieldDecl, FunctionDecl, Ident, If, SourceFile, Span,
    Statement, StatementKind, StructDecl, TypeRef, TypeRefKind, ValDecl, While,
};

use crate::lexer::{Token, TokenKind, lex};

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

    fn parse_decl(&mut self) -> Result<Decl, Diagnostic> {
        match &self.peek().kind {
            TokenKind::Fun => Ok(Decl::Function(self.parse_function()?)),
            TokenKind::Struct => Ok(Decl::Struct(self.parse_struct()?)),
            TokenKind::Ident(text) if text == "enum" => Err(Diagnostic::at(
                self.peek().span,
                "enums are not supported yet (milestone M2)",
            )),
            _ => self.unexpected("`fun` or `struct`"),
        }
    }

    /// `fun <name>() { ... }` — M2 has no parameters and no return type
    /// annotation.
    fn parse_function(&mut self) -> Result<FunctionDecl, Diagnostic> {
        let fun = self.expect("`fun`", |k| matches!(k, TokenKind::Fun))?;
        let name = self.expect_ident("function name")?;
        self.expect("`(`", |k| matches!(k, TokenKind::LParen))?;
        if !matches!(self.peek().kind, TokenKind::RParen) {
            return Err(Diagnostic::at(
                self.peek().span,
                "parameters are not supported yet (milestone M2)",
            ));
        }
        self.bump(); // `)`
        if matches!(self.peek().kind, TokenKind::Colon) {
            return Err(Diagnostic::at(
                self.peek().span,
                "return type annotations are not supported yet (milestone M2)",
            ));
        }
        let body = self.parse_block()?;
        Ok(FunctionDecl {
            name,
            span: Span::new(fun.span.start, body.span.end),
            body,
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
                    "field default values are not supported yet (milestone M2)",
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
                "struct member declarations are not supported yet (milestone M2)",
            ));
        }
        Ok(StructDecl {
            name,
            fields,
            span: Span::new(keyword.span.start, close.span.end),
        })
    }

    /// A type annotation: a named type, `Unit` (also written `()`), or a
    /// tuple type `(T1, T2, ...)`. Disambiguation mirrors expressions
    /// (spec section 4.3): `(T)` is just `T` in parentheses, `(T,)` a
    /// 1-tuple type.
    pub(crate) fn parse_type_ref(&mut self) -> Result<TypeRef, Diagnostic> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Ident(text) => {
                self.pos += 1;
                if text == "Unit" {
                    Ok(TypeRef {
                        kind: TypeRefKind::Unit,
                        span: token.span,
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

    fn parse_block(&mut self) -> Result<Block, Diagnostic> {
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
    fn expect_statement_end(&mut self) -> Result<(), Diagnostic> {
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
            TokenKind::While => self.parse_while(),
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                Ok(Statement {
                    span: block.span,
                    kind: StatementKind::Block(block),
                })
            }
            TokenKind::Ident(text) if text == "for" => Err(Diagnostic::at(
                self.peek().span,
                "`for` loops are not supported yet (milestone M2)",
            )),
            // `name = ...` (a single `=`, not `==`) assigns to a local `var`.
            TokenKind::Ident(_) if matches!(self.tokens[self.pos + 1].kind, TokenKind::Equal) => {
                self.parse_assign()
            }
            _ => {
                let expr = self.parse_expr()?;
                if matches!(self.peek().kind, TokenKind::Equal) {
                    if matches!(expr, Expr::FieldAccess(_)) {
                        return Err(Diagnostic::at(
                            self.peek().span,
                            "field assignment is not supported (value types are immutable)",
                        ));
                    }
                    return self.unexpected("`;` or newline after statement");
                }
                Ok(Statement {
                    span: expr.span(),
                    kind: StatementKind::Expr(expr),
                })
            }
        }
    }

    /// `(val|var) <name> (: <type>)? = <expr>` — the initializer is
    /// mandatory in M2.
    fn parse_val_decl(&mut self) -> Result<Statement, Diagnostic> {
        let keyword = self.bump();
        let mutable = matches!(keyword.kind, TokenKind::Var);
        let name = self.expect_ident("variable name")?;
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
                name,
                ty,
                init,
                span,
            }),
        })
    }

    /// `<name> = <expr>` — M2 assigns to plain local variables only.
    fn parse_assign(&mut self) -> Result<Statement, Diagnostic> {
        let target = self.expect_ident("assignment target")?;
        self.bump(); // `=`
        let value = self.parse_expr()?;
        let span = Span::new(target.span.start, value.span().end);
        Ok(Statement {
            span,
            kind: StatementKind::Assign(Assign {
                target,
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
