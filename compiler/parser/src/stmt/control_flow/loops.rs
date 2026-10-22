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
        let body = self.parse_control_body()?;
        self.expect("`while` after do body", |kind| {
            matches!(kind, TokenKind::While)
        })?;
        self.expect("`(`", |kind| matches!(kind, TokenKind::LParen))?;
        let cond = self.parse_expr()?;
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
        let keyword = self.bump();
        let start = label
            .as_ref()
            .map_or(keyword.span.start, |label| label.span.start);
        self.expect("`(`", |kind| matches!(kind, TokenKind::LParen))?;
        let pattern = self.parse_pattern()?;
        self.expect("`in`", |kind| matches!(kind, TokenKind::In))?;
        let iterable = self.parse_expr()?;
        self.expect("`)`", |kind| matches!(kind, TokenKind::RParen))?;
        let body = self.parse_control_body()?;
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
