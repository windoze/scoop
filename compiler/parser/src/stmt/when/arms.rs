use super::*;
use scoop_syntax::SyntaxKind;

impl Parser {
    pub(super) fn parse_when_arm(&mut self, has_subject: bool) -> Result<WhenArm, Diagnostic> {
        self.with_node(SyntaxKind::WhenArm, |parser| {
            parser.parse_when_arm_inner(has_subject)
        })
    }

    fn parse_when_arm_inner(&mut self, has_subject: bool) -> Result<WhenArm, Diagnostic> {
        let start = self.peek().span.start;
        let condition = self.parse_when_arm_condition(has_subject)?;
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
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect("`->`", |kind| matches!(kind, TokenKind::Arrow))?;
        let body = self.parse_arm_body()?;
        self.expect_statement_end()?;
        Ok(WhenArm {
            span: Span::new(start, body.span.end),
            condition,
            guard,
            body,
        })
    }
}
