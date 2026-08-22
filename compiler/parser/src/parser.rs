//! Recursive-descent parser for the M3 subset: token vector -> AST.
//!
//! M3 is fail-fast (see `docs/milestone2/DESIGN.md` section 5): the first
//! error aborts parsing with a single spanned diagnostic. Constructs that
//! are lexically recognizable but outside the subset (`enum`, `when`,
//! `for`, string interpolation, field assignment) get dedicated "not
//! supported" diagnostics rather than generic syntax errors. Expression
//! parsing lives in `expr.rs`.

use scoop_ast::{
    Assign, Block, Decl, Diagnostic, Expr, FieldDecl, FunctionBody, FunctionDecl, Ident, If, Param,
    SourceFile, Span, Statement, StatementKind, StructDecl, TypeRef, TypeRefKind, ValDecl, While,
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
                "enums are not supported yet (milestone M3)",
            )),
            _ => self.unexpected("`fun` or `struct`"),
        }
    }

    /// `fun <T, ...>? <name>(<param>, ...)?: <ret>? <body>` — the type
    /// parameter list sits between `fun` and the name (a `<` right after
    /// `fun` is unambiguous here), parameters carry mandatory type
    /// annotations, and the return type defaults to `Unit` when absent.
    /// The body is a block or an expression body (`= expr`).
    fn parse_function(&mut self) -> Result<FunctionDecl, Diagnostic> {
        let fun = self.expect("`fun`", |k| matches!(k, TokenKind::Fun))?;
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
        self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
        let return_ty = if matches!(self.peek().kind, TokenKind::Colon) {
            self.bump();
            Some(self.parse_type_ref()?)
        } else {
            None
        };
        let (body, end) = match self.peek().kind {
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
            _ => return self.unexpected("`{` or `=`"),
        };
        Ok(FunctionDecl {
            name,
            type_params,
            params,
            return_ty,
            body,
            span: Span::new(fun.span.start, end),
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
            TokenKind::Return => self.parse_return(),
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                Ok(Statement {
                    span: block.span,
                    kind: StatementKind::Block(block),
                })
            }
            TokenKind::Ident(text) if text == "for" => Err(Diagnostic::at(
                self.peek().span,
                "`for` loops are not supported yet (milestone M3)",
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
