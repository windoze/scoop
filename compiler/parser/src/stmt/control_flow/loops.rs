use super::*;

impl Parser {
    /// The condition is always parenthesized, including a labeled loop.
    pub(crate) fn parse_while(
        &mut self,
        label: Option<ast::Ident>,
    ) -> Result<Statement, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::WhileStmt, |parser| {
            parser.parse_while_inner(label)
        })
    }

    fn parse_while_inner(&mut self, label: Option<ast::Ident>) -> Result<Statement, Diagnostic> {
        let checkpoint = self.checkpoint();
        let keyword = self.bump();
        let start = label
            .as_ref()
            .map_or(keyword.span.start, |label| label.span.start);
        self.expect("`(`", |kind| matches!(kind, TokenKind::LParen))?;
        let cond =
            self.parse_statement_operand(start, || ast::RecoveredOperandContext::WhileCondition {
                label: label.clone(),
            })?;
        self.expect("`)`", |kind| matches!(kind, TokenKind::RParen))?;
        let complete_header = self.fragment_is_complete(checkpoint);
        let (result, recovered) = self.parse_recoverable_control_body();
        if complete_header && let Some(body) = recovered {
            self.record_recovered_while(ast::RecoveredWhile {
                block: None,
                start,
                label: label.clone(),
                condition: Box::new(cond.clone()),
                body,
            });
        }
        let body = result?;
        let span = Span::new(start, body.span.end);
        Ok(Statement {
            span,
            kind: StatementKind::While(While {
                loop_kind: ast::LoopKind::While,
                label,
                cond,
                body,
                span,
            }),
        })
    }

    pub(crate) fn parse_do_while(
        &mut self,
        label: Option<ast::Ident>,
    ) -> Result<Statement, Diagnostic> {
        let keyword = self.bump();
        let start = label
            .as_ref()
            .map_or(keyword.span.start, |label| label.span.start);
        let (result, recovered) = self.parse_recoverable_control_body();
        if let Some(body) = recovered {
            self.record_recovered_do_while(ast::RecoveredDoWhile {
                block: None,
                start,
                label: label.clone(),
                part: ast::RecoveredDoWhilePart::Body(body),
            });
        }
        let body = result?;
        self.expect("`while` after do body", |kind| {
            matches!(kind, TokenKind::While)
        })?;
        self.expect("`(`", |kind| matches!(kind, TokenKind::LParen))?;
        let expression_start = self.peek().span.start;
        let condition_checkpoint = self.checkpoint();
        let result = self.parse_expr();
        if recovered.is_none()
            && (result.is_err() || !self.fragment_is_complete(condition_checkpoint))
        {
            self.record_recovered_do_while(ast::RecoveredDoWhile {
                block: None,
                start,
                label: label.clone(),
                part: ast::RecoveredDoWhilePart::Condition {
                    body: Box::new(body.clone()),
                    expression_start,
                },
            });
        }
        let cond = result?;
        let end = self
            .expect("`)`", |kind| matches!(kind, TokenKind::RParen))?
            .span
            .end;
        let span = Span::new(start, end);
        Ok(Statement {
            kind: StatementKind::While(While {
                loop_kind: ast::LoopKind::DoWhile,
                label,
                cond,
                body,
                span,
            }),
            span,
        })
    }

    /// Patterns share the ordinary grammar; irrefutability is checked later.
    pub(crate) fn parse_for(&mut self, label: Option<ast::Ident>) -> Result<Statement, Diagnostic> {
        self.with_node(scoop_syntax::SyntaxKind::ForStmt, |parser| {
            parser.parse_for_inner(label)
        })
    }

    fn parse_for_inner(&mut self, label: Option<ast::Ident>) -> Result<Statement, Diagnostic> {
        let checkpoint = self.checkpoint();
        let keyword = self.bump();
        let start = label
            .as_ref()
            .map_or(keyword.span.start, |label| label.span.start);
        self.expect("`(`", |kind| matches!(kind, TokenKind::LParen))?;
        let pattern = self.parse_pattern()?;
        self.expect("`in`", |kind| matches!(kind, TokenKind::In))?;
        let iterable =
            self.parse_statement_operand(start, || ast::RecoveredOperandContext::ForIterable)?;
        self.expect("`)`", |kind| matches!(kind, TokenKind::RParen))?;
        let complete_header = self.fragment_is_complete(checkpoint);
        let (result, recovered) = self.parse_recoverable_control_body();
        if complete_header && let Some(body) = recovered {
            self.record_recovered_for(ast::RecoveredFor {
                block: None,
                span: Span::new(
                    start,
                    result
                        .as_ref()
                        .map_or(self.peek().span.start, |body| body.span.end),
                ),
                label: label.clone(),
                pattern: Box::new(pattern.clone()),
                iterable: Box::new(iterable.clone()),
                body,
            });
        }
        let body = result?;
        let span = Span::new(start, body.span.end);
        Ok(Statement {
            kind: StatementKind::For(For {
                label,
                pattern,
                iterable,
                body,
                span,
            }),
            span,
        })
    }
}
