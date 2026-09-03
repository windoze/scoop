//! Structured conditional, loop, pattern dispatch, and exception syntax.

use scoop_ast::{
    Block, CatchClause, Diagnostic, Expr, If, Span, Statement, StatementKind, Try, When, WhenArm,
    While,
};

use crate::lexer::TokenKind;
use crate::parser::Parser;
use crate::pattern::pattern_span;

impl Parser {
    /// `if (<cond>) { ... } (else { ... })?` — the condition is always
    /// parenthesized. `else if` chains get no special treatment (M2 design
    /// section 6): `else` must be followed by a block.
    pub(super) fn parse_if(&mut self) -> Result<Statement, Diagnostic> {
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
    pub(super) fn parse_when(&mut self) -> Result<Statement, Diagnostic> {
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

    /// `while (<cond>) { ... }` — the condition is always parenthesized.
    pub(super) fn parse_while(&mut self) -> Result<Statement, Diagnostic> {
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
    pub(super) fn parse_try(&mut self) -> Result<Statement, Diagnostic> {
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
}
