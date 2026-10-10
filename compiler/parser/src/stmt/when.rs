//! Explicit pattern dispatch syntax and arm bodies.

use scoop_ast::{Block, Diagnostic, Expr, Span, Statement, StatementKind, When, WhenArm};

use crate::lexer::TokenKind;
use crate::parser::Parser;

impl Parser {
    /// `when (<subject>) { <arm>* (else -> <block>)? }` — statement-level
    /// pattern matching (spec chapter 5). An arm is
    /// `case <pattern> (if (<guard>))? -> <body>` where the body is a block, a
    /// single expression statement (`case Red -> println("red")`, spec 5.1), or
    /// an M22 loop jump; arms end like statements, and `else` must be last.
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
                    let body = self.parse_arm_body()?;
                    self.expect_statement_end()?;
                    else_body = Some(body);
                    // `else` must be the last arm.
                    break self.expect("`}`", |k| matches!(k, TokenKind::RBrace))?;
                }
                _ => {
                    let prefix = self.peek().span;
                    if !matches!(&self.peek().kind, TokenKind::Ident(name) if name == "case") {
                        return Err(Diagnostic::at(
                            prefix,
                            "match patterns require `case` before the pattern",
                        ));
                    }
                    self.bump();
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
                    let span = Span::new(prefix.start, body.span.end);
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

    /// A `when` arm body: a block, a single expression statement, or an M22
    /// loop jump, wrapped in a synthetic block (the AST keeps `Block` for all
    /// three forms).
    fn parse_arm_body(&mut self) -> Result<Block, Diagnostic> {
        if matches!(self.peek().kind, TokenKind::LBrace) {
            return self.parse_block();
        }
        if matches!(&self.peek().kind, TokenKind::Break | TokenKind::Continue) {
            return self.parse_loop_jump_block();
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
}
