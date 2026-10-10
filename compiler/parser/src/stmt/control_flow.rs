//! Structured conditional, loop, pattern dispatch, and exception syntax.

use scoop_ast::{
    Block, CatchClause, Diagnostic, Expr, For, If, Span, Statement, StatementKind, Try, While,
};

use crate::lexer::TokenKind;
use crate::parser::Parser;

impl Parser {
    /// `if (<cond>) { ... } (else { ... })?` — the condition is always
    /// parenthesized. M22 additionally permits an unbraced `break` or
    /// `continue` as either branch. Other unbraced bodies, including
    /// `else if`, remain outside the accepted syntax.
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
        let then_block = self.parse_if_branch_body()?;
        let mut end = then_block.span.end;
        let else_block = if matches!(self.peek().kind, TokenKind::Else) {
            self.bump();
            let block = self.parse_if_branch_body()?;
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

    fn parse_if_branch_body(&mut self) -> Result<Block, Diagnostic> {
        if matches!(self.peek().kind, TokenKind::LBrace) {
            return self.parse_block();
        }
        if matches!(&self.peek().kind, TokenKind::Break | TokenKind::Continue) {
            return self.parse_loop_jump_block();
        }
        self.parse_block()
    }

    pub(super) fn parse_loop_jump_block(&mut self) -> Result<Block, Diagnostic> {
        let statement = self.parse_loop_jump()?;
        let span = statement.span;
        Ok(Block {
            statements: vec![statement],
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

    /// `for (<pattern> in <iterable>) { ... }`. Pattern syntax is deliberately
    /// shared with `val`/lambda/`when`; irrefutability is a later typed check.
    pub(super) fn parse_for(&mut self) -> Result<Statement, Diagnostic> {
        let keyword = self.bump(); // `for`
        self.expect("`(`", |kind| matches!(kind, TokenKind::LParen))?;
        let pattern = self.parse_pattern()?;
        self.expect("`in`", |kind| matches!(kind, TokenKind::In))?;
        let iterable = self.parse_expr()?;
        self.expect("`)`", |kind| matches!(kind, TokenKind::RParen))?;
        let body = self.parse_block()?;
        let span = Span::new(keyword.span.start, body.span.end);
        Ok(Statement {
            kind: StatementKind::For(For {
                pattern,
                iterable,
                body,
                span,
            }),
            span,
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
