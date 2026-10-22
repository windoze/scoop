//! Structured conditional, loop, pattern dispatch, and exception syntax.

use scoop_ast::{
    self as ast, Block, CatchClause, Diagnostic, Expr, For, Span, Statement, StatementKind, Try,
    While,
};

use crate::lexer::TokenKind;
use crate::parser::Parser;

mod conditionals;
mod loops;

impl Parser {
    pub(crate) fn parse_control_body(&mut self) -> Result<Block, Diagnostic> {
        if matches!(self.peek().kind, TokenKind::LBrace) {
            return self.parse_block();
        }
        let statement = self.parse_statement()?;
        let span = statement.span;
        Ok(Block {
            statements: vec![statement],
            span,
        })
    }

    fn parse_recoverable_control_body(
        &mut self,
    ) -> (Result<Block, Diagnostic>, Option<ast::RecoveredControlBody>) {
        let checkpoint = self.checkpoint();
        let start = self.peek().span.start;
        let body = if matches!(self.peek().kind, TokenKind::LBrace) {
            ast::RecoveredControlBody::Block(ast::BodyBlockId(start))
        } else {
            ast::RecoveredControlBody::Statement(start)
        };
        let result = self.parse_control_body();
        let recovered = (result.is_err() || !self.fragment_is_complete(checkpoint)).then_some(body);
        (result, recovered)
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
        self.with_node(scoop_syntax::SyntaxKind::TryExpr, |parser| {
            parser.parse_try_node_inner()
        })
    }

    fn parse_try_node_inner(&mut self) -> Result<Try, Diagnostic> {
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
