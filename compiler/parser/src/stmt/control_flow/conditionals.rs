use super::*;
use scoop_ast::{self as ast, If};

impl Parser {
    /// Recursing through the body binds `else` to the nearest unmatched `if`.
    pub(crate) fn parse_if(&mut self) -> Result<Statement, Diagnostic> {
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
        self.with_node(scoop_syntax::SyntaxKind::IfExpr, |parser| {
            parser.parse_if_node_inner()
        })
    }

    fn parse_if_node_inner(&mut self) -> Result<If, Diagnostic> {
        let checkpoint = self.checkpoint();
        let keyword = self.bump(); // `if`
        self.expect("`(`", |k| matches!(k, TokenKind::LParen))?;
        let cond = self.parse_statement_operand(keyword.span.start, || {
            ast::RecoveredOperandContext::IfCondition
        })?;
        self.expect("`)`", |k| matches!(k, TokenKind::RParen))?;
        let complete_header = self.fragment_is_complete(checkpoint);
        let mut branches = Vec::new();
        let result = self.parse_if_bodies(&mut branches);
        if complete_header && !branches.is_empty() {
            self.record_recovered_if(ast::RecoveredIf {
                block: None,
                start: keyword.span.start,
                condition: Box::new(cond.clone()),
                branches,
            });
        }
        let (then_block, else_block, end) = result?;
        Ok(If {
            cond,
            then_block,
            else_block,
            span: Span::new(keyword.span.start, end),
        })
    }

    fn parse_if_bodies(
        &mut self,
        recovered: &mut Vec<ast::RecoveredIfBranch>,
    ) -> Result<(Block, Option<Block>, u32), Diagnostic> {
        let then_block = self.parse_if_branch(true, recovered)?;
        let mut end = then_block.span.end;
        let after_separator = self.tokens[self.pos..]
            .iter()
            .position(|token| !matches!(token.kind, TokenKind::Semicolon))
            .map(|offset| self.pos + offset)
            .expect("token stream ends in EOF");
        if matches!(self.tokens[after_separator].kind, TokenKind::Else) {
            self.pos = after_separator;
        }
        let else_block = if matches!(self.peek().kind, TokenKind::Else) {
            self.bump();
            let block = self.parse_if_branch(false, recovered)?;
            end = block.span.end;
            Some(block)
        } else {
            None
        };
        Ok((then_block, else_block, end))
    }

    fn parse_if_branch(
        &mut self,
        on_true: bool,
        recovered: &mut Vec<ast::RecoveredIfBranch>,
    ) -> Result<Block, Diagnostic> {
        let (result, body) = self.parse_recoverable_control_body();
        if let Some(body) = body {
            recovered.push(ast::RecoveredIfBranch { on_true, body });
        }
        result
    }
}
