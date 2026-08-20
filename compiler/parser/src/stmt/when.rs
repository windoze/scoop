//! Ordered ordinary conditions and explicit pattern dispatch.

use scoop_ast::{
    Block, Diagnostic, Expr, NonEmptyVec, Span, Statement, StatementKind, When, WhenArm,
    WhenArmCondition, WhenCondition, WhenSubject,
};

use crate::lexer::{Token, TokenKind};
use crate::parser::Parser;

impl Parser {
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
        let keyword = self.bump();
        let subject = self.parse_when_subject()?;
        let has_subject = !matches!(subject, WhenSubject::Absent);
        self.expect("`{`", |kind| matches!(kind, TokenKind::LBrace))?;
        let mut arms = Vec::new();
        let mut else_body = None;
        let close = loop {
            match self.peek().kind {
                TokenKind::RBrace => break self.bump(),
                TokenKind::Eof => return self.unexpected("`}`"),
                _ => {
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
                        if matches!(&condition, WhenArmCondition::Conditions(values) if values.len() > 1)
                        {
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
                    if matches!(condition, WhenArmCondition::Else) && guard.is_none() {
                        else_body = Some(body);
                        break self.expect("`}`", |kind| matches!(kind, TokenKind::RBrace))?;
                    }
                    arms.push(WhenArm {
                        span: Span::new(start, body.span.end),
                        condition,
                        guard,
                        body,
                    });
                }
            }
        };
        Ok(When {
            subject,
            arms,
            else_body,
            span: Span::new(keyword.span.start, close.span.end),
        })
    }

    fn parse_when_subject(&mut self) -> Result<WhenSubject, Diagnostic> {
        if matches!(self.peek().kind, TokenKind::LBrace) {
            return Ok(WhenSubject::Absent);
        }
        self.expect("`(`", |kind| matches!(kind, TokenKind::LParen))?;
        let subject = match self.peek().kind {
            TokenKind::Var => {
                return Err(Diagnostic::at(
                    self.peek().span,
                    "a when subject declaration must use `val`, not `var`",
                ));
            }
            TokenKind::Val => {
                let declaration = self.parse_val_decl()?;
                let StatementKind::ValDecl(value) = declaration.kind else {
                    return Err(Diagnostic::at(
                        declaration.span,
                        "a when subject declaration cannot be delegated",
                    ));
                };
                WhenSubject::Declaration(Box::new(value))
            }
            _ => WhenSubject::Expression(self.parse_expr()?),
        };
        self.expect("`)`", |kind| matches!(kind, TokenKind::RParen))?;
        Ok(subject)
    }

    fn parse_when_arm_condition(
        &mut self,
        has_subject: bool,
    ) -> Result<WhenArmCondition, Diagnostic> {
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
        let first = self.parse_when_condition(has_subject)?;
        let mut rest = Vec::new();
        while matches!(self.peek().kind, TokenKind::Comma) {
            self.bump();
            if matches!(self.peek().kind, TokenKind::Arrow | TokenKind::If) {
                break;
            }
            rest.push(self.parse_when_condition(has_subject)?);
        }
        Ok(WhenArmCondition::Conditions(NonEmptyVec::new(first, rest)))
    }

    fn parse_when_condition(&mut self, has_subject: bool) -> Result<WhenCondition, Diagnostic> {
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
            let negated = matches!(self.bump().kind, TokenKind::BangIn);
            let collection = self.parse_expr()?;
            return Ok(WhenCondition::In {
                span: Span::new(start, collection.span().end),
                collection,
                negated,
            });
        }
        Ok(WhenCondition::Expression(self.parse_expr()?))
    }

    fn parse_arm_body(&mut self) -> Result<Block, Diagnostic> {
        if matches!(self.peek().kind, TokenKind::LBrace) {
            return self.parse_block();
        }
        if matches!(self.peek().kind, TokenKind::Val | TokenKind::Var) {
            return self.unexpected("expression");
        }
        let nesting = self
            .arm_body_nesting
            .replace(self.expression_nesting[self.pos]);
        let statement = self.parse_statement();
        self.arm_body_nesting = nesting;
        let statement = statement?;
        let span = statement.span;
        Ok(Block {
            statements: vec![statement],
            span,
        })
    }
}

impl Parser {
    /// Token positions may be rewound by generic-call probing. An immutable
    /// nesting index keeps arm boundaries correct across those rewinds.
    pub(crate) fn expression_nesting(tokens: &[Token]) -> Vec<usize> {
        let mut depth = 0usize;
        tokens
            .iter()
            .map(|token| {
                let before = depth;
                match token.kind {
                    TokenKind::LParen
                    | TokenKind::LBracket
                    | TokenKind::LBrace
                    | TokenKind::InterpolationStart => depth += 1,
                    TokenKind::RParen
                    | TokenKind::RBracket
                    | TokenKind::RBrace
                    | TokenKind::InterpolationEnd => depth = depth.saturating_sub(1),
                    _ => {}
                }
                before
            })
            .collect()
    }

    pub(crate) fn at_arm_body_newline(&self) -> bool {
        self.peek().newline_before
            && self.arm_body_nesting == Some(self.expression_nesting[self.pos])
    }
}
