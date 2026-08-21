//! Recursive-descent parser for the M1 subset: token vector -> AST.
//!
//! M1 is fail-fast (see `docs/milestone1/DESIGN.md` section 2.2): the first
//! error aborts parsing with a single spanned diagnostic. Constructs that
//! are lexically recognizable but outside the subset (parameters, return
//! type annotations, `val`/`var`) get dedicated "not supported yet"
//! diagnostics rather than generic syntax errors.

use scoop_ast::{
    Block, CallExpr, Diagnostic, Expr, FunctionDecl, Ident, SourceFile, Span, Statement,
    StatementKind,
};

use crate::lexer::{Token, TokenKind, lex};

pub(crate) fn parse_file(source: &str) -> Result<SourceFile, Diagnostic> {
    let tokens = lex(source)?;
    let mut parser = Parser { tokens, pos: 0 };
    let mut functions = Vec::new();
    while !parser.at_eof() {
        functions.push(parser.parse_function()?);
    }
    Ok(SourceFile {
        functions,
        span: Span::new(0, source.len() as u32),
    })
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn bump(&mut self) -> Token {
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
    fn unexpected<T>(&self, what: &str) -> Result<T, Diagnostic> {
        let token = self.peek();
        Err(Diagnostic::at(
            token.span,
            format!("expected {what}, found {}", token.describe()),
        ))
    }

    fn expect(
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

    fn expect_ident(&mut self, what: &str) -> Result<Ident, Diagnostic> {
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

    /// `fun <name>() { ... }` — M1 has no parameters and no return type
    /// annotation.
    fn parse_function(&mut self) -> Result<FunctionDecl, Diagnostic> {
        let fun = self.expect("`fun`", |k| matches!(k, TokenKind::Fun))?;
        let name = self.expect_ident("function name")?;
        self.expect("`(`", |k| matches!(k, TokenKind::LParen))?;
        if !matches!(self.peek().kind, TokenKind::RParen) {
            return Err(Diagnostic::at(
                self.peek().span,
                "parameters are not supported yet (milestone M1)",
            ));
        }
        self.bump(); // `)`
        if matches!(self.peek().kind, TokenKind::Colon) {
            return Err(Diagnostic::at(
                self.peek().span,
                "return type annotations are not supported yet (milestone M1)",
            ));
        }
        let body = self.parse_block()?;
        Ok(FunctionDecl {
            name,
            span: Span::new(fun.span.start, body.span.end),
            body,
        })
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
        if let TokenKind::Ident(text) = &self.peek().kind {
            if text == "val" || text == "var" {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "variable declarations are not supported yet (milestone M1)",
                ));
            }
        }
        let expr = self.parse_expr()?;
        Ok(Statement {
            span: expr.span(),
            kind: StatementKind::Expr(expr),
        })
    }

    /// M1 expressions: string literals and direct calls `name(args...)`.
    fn parse_expr(&mut self) -> Result<Expr, Diagnostic> {
        let token = self.peek().clone();
        match token.kind {
            TokenKind::Str(value) => {
                self.pos += 1;
                Ok(Expr::StringLiteral {
                    value,
                    span: token.span,
                })
            }
            TokenKind::Ident(text) => {
                self.pos += 1;
                let callee = Ident {
                    text,
                    span: token.span,
                };
                self.expect("`(`", |k| matches!(k, TokenKind::LParen))?;
                let mut args = Vec::new();
                if !matches!(self.peek().kind, TokenKind::RParen) {
                    loop {
                        args.push(self.parse_expr()?);
                        if matches!(self.peek().kind, TokenKind::Comma) {
                            self.bump();
                        } else {
                            break;
                        }
                    }
                }
                let close = self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
                Ok(Expr::Call(CallExpr {
                    callee,
                    args,
                    span: Span::new(token.span.start, close.span.end),
                }))
            }
            _ => self.unexpected("expression"),
        }
    }
}
