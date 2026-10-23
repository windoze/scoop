use super::*;

impl Parser {
    pub(super) fn parse_when_arm_condition(
        &mut self,
        has_subject: bool,
        recovery: &mut Option<ast::RecoveredWhenArm>,
    ) -> Result<WhenArmCondition, Diagnostic> {
        let start = self.peek().span.start;
        if matches!(self.peek().kind, TokenKind::Else) {
            self.bump();
            return Ok(WhenArmCondition::Else);
        }
        if matches!(&self.peek().kind, TokenKind::Ident(name) if name == "case") {
            let keyword = self.bump();
            if !has_subject {
                return Err(Diagnostic::at(
                    keyword.span,
                    "case patterns require a when subject",
                ));
            }
            let pattern = self.parse_pattern()?;
            if matches!(self.peek().kind, TokenKind::Comma) {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "a case arm accepts one pattern; use separate arms for alternatives",
                ));
            }
            return Ok(WhenArmCondition::Case(pattern));
        }
        if matches!(&self.peek().kind, TokenKind::Ident(name) if name == "_")
            || matches!(self.peek().kind, TokenKind::DotDot)
        {
            return Err(Diagnostic::at(
                self.peek().span,
                "match patterns require `case` before the pattern",
            ));
        }
        let mut values = Vec::new();
        loop {
            let checkpoint = self.checkpoint();
            let mut expression_start = self.peek().span.start;
            let result = self.parse_when_condition(has_subject, &mut expression_start);
            if recovery.is_none() && (result.is_err() || !self.fragment_is_complete(checkpoint)) {
                *recovery = Some(ast::RecoveredWhenArm::Condition {
                    previous: values.clone(),
                    expression_start,
                    span: Span::new(start, self.peek().span.start),
                });
            }
            values.push(result?);
            if !matches!(self.peek().kind, TokenKind::Comma) {
                break;
            }
            self.bump();
            if matches!(self.peek().kind, TokenKind::Arrow | TokenKind::If) {
                break;
            }
        }
        let mut values = values.into_iter();
        let first = values
            .next()
            .expect("one condition is parsed before its separator");
        Ok(WhenArmCondition::Conditions(NonEmptyVec::new(
            first,
            values.collect(),
        )))
    }

    fn parse_when_condition(
        &mut self,
        has_subject: bool,
        expression_start: &mut u32,
    ) -> Result<WhenCondition, Diagnostic> {
        let start = self.peek().span.start;
        let negated_is = matches!(self.peek().kind, TokenKind::Bang)
            && matches!(self.tokens[self.pos + 1].kind, TokenKind::Is);
        if matches!(self.peek().kind, TokenKind::Is) || negated_is {
            if !has_subject {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "an abbreviated `is` condition requires a when subject",
                ));
            }
            self.bump();
            if negated_is {
                self.bump();
            }
            let ty = self.parse_type_ref()?;
            return Ok(WhenCondition::Is {
                span: Span::new(start, ty.span.end),
                ty,
                negated: negated_is,
            });
        }
        if matches!(self.peek().kind, TokenKind::In | TokenKind::BangIn) {
            if !has_subject {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "an abbreviated `in` condition requires a when subject",
                ));
            }
            let operator = self.bump();
            let negated = matches!(operator.kind, TokenKind::BangIn);
            *expression_start = self.peek().span.start;
            let collection = self.parse_expr()?;
            return Ok(WhenCondition::In {
                span: Span::new(start, collection.span().end),
                collection,
                negated,
                operator_span: operator.span,
            });
        }
        Ok(WhenCondition::Expression(self.parse_expr()?))
    }
}
