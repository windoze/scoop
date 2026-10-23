use super::*;
use scoop_syntax::SyntaxKind;

impl Parser {
    pub(super) fn parse_when_arm(
        &mut self,
        has_subject: bool,
    ) -> (Result<WhenArm, Diagnostic>, Option<ast::RecoveredWhenArm>) {
        let checkpoint = self.checkpoint();
        let start = self.peek().span.start;
        let mut recovery = None;
        let result = self.with_node(SyntaxKind::WhenArm, |parser| {
            parser.parse_when_arm_inner(has_subject, &mut recovery)
        });
        if recovery.is_none() && !self.fragment_is_complete(checkpoint) {
            recovery = Some(ast::RecoveredWhenArm::Unavailable(Span::new(
                start,
                self.peek().span.start,
            )));
        }
        (result, recovery)
    }

    fn parse_when_arm_inner(
        &mut self,
        has_subject: bool,
        recovery: &mut Option<ast::RecoveredWhenArm>,
    ) -> Result<WhenArm, Diagnostic> {
        let checkpoint = self.checkpoint();
        let start = self.peek().span.start;
        let condition = self.parse_when_arm_condition(has_subject, recovery)?;
        let guard = if matches!(self.peek().kind, TokenKind::If) {
            let keyword = self.bump();
            if !has_subject {
                return Err(Diagnostic::at(
                    keyword.span,
                    "when guards require a subject; combine conditions with `&&`",
                ));
            }
            if matches!(&condition, WhenArmCondition::Conditions(values) if values.len() > 1) {
                return Err(Diagnostic::at(
                    keyword.span,
                    "a when arm with multiple conditions cannot have a guard",
                ));
            }
            let complete_primary = self.fragment_is_complete(checkpoint);
            let guard_checkpoint = self.checkpoint();
            let expression_start = self.peek().span.start;
            let result = self.parse_expr();
            if recovery.is_none()
                && complete_primary
                && (result.is_err() || !self.fragment_is_complete(guard_checkpoint))
            {
                *recovery = Some(ast::RecoveredWhenArm::Guard {
                    condition: condition.clone(),
                    expression_start,
                    span: Span::new(start, self.peek().span.start),
                });
            }
            Some(result?)
        } else {
            None
        };
        self.expect("`->`", |kind| matches!(kind, TokenKind::Arrow))?;
        let complete_header = self.fragment_is_complete(checkpoint);
        let body_checkpoint = self.checkpoint();
        let body_start = self.peek().span.start;
        let entry = if matches!(self.peek().kind, TokenKind::LBrace) {
            ast::RecoveredControlBody::Block(ast::BodyBlockId(body_start))
        } else {
            ast::RecoveredControlBody::Statement(body_start)
        };
        let result = self.parse_arm_body();
        if recovery.is_none()
            && complete_header
            && (result.is_err() || !self.fragment_is_complete(body_checkpoint))
        {
            *recovery = Some(ast::RecoveredWhenArm::Head {
                condition: condition.clone(),
                guard: guard.clone(),
                body: Some(entry),
                span: Span::new(start, self.peek().span.start),
            });
        }
        let body = result?;
        self.expect_statement_end()?;
        Ok(WhenArm {
            span: Span::new(start, body.span.end),
            condition,
            guard,
            body,
        })
    }
}
