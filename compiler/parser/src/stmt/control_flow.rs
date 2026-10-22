//! Structured conditional, loop, pattern dispatch, and exception syntax.

use scoop_ast::{
    self as ast, Block, CatchClause, Diagnostic, Expr, For, Span, Statement, StatementKind, Try,
    While,
};

use crate::lexer::TokenKind;
use crate::parser::Parser;

mod conditionals;
mod exceptions;
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
}
