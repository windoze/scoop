//! Ordered ordinary conditions and explicit pattern dispatch.

use scoop_ast::{
    self as ast, Block, Diagnostic, Expr, NonEmptyVec, Span, Statement, StatementKind, When,
    WhenArm, WhenArmCondition, WhenCondition, WhenSubject,
};

use crate::lexer::{Token, TokenKind};
use crate::parser::Parser;

mod arms;
mod conditions;
mod subject;

enum ParsedArm {
    Projected {
        arm: WhenArm,
        recovery: Option<Box<ast::RecoveredWhenArm>>,
    },
    Failed(ast::RecoveredWhenArm),
}

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
        self.with_node(scoop_syntax::SyntaxKind::WhenExpr, |parser| {
            parser.parse_when_node_inner()
        })
    }

    fn parse_when_node_inner(&mut self) -> Result<When, Diagnostic> {
        let checkpoint = self.checkpoint();
        let keyword = self.bump();
        let subject = self.parse_when_subject(keyword.span.start)?;
        let complete_subject = self.fragment_is_complete(checkpoint);
        let has_subject = !matches!(subject, WhenSubject::Absent);
        self.expect("`{`", |kind| matches!(kind, TokenKind::LBrace))?;
        let mut parsed = Vec::new();
        let result = self.parse_when_arms(has_subject, &mut parsed);
        let recover =
            complete_subject && (result.is_err() || !self.fragment_is_complete(checkpoint));
        let mut arms = Vec::new();
        let mut else_body = None;
        let mut recovered = Vec::new();
        for entry in parsed {
            match entry {
                ParsedArm::Projected { arm, recovery } => {
                    if recover {
                        recovered.push(recovery.map(|input| *input).unwrap_or_else(|| {
                            ast::RecoveredWhenArm::Head {
                                condition: arm.condition.clone(),
                                guard: arm.guard.clone(),
                                body: None,
                                span: arm.span,
                            }
                        }));
                    }
                    if matches!(arm.condition, WhenArmCondition::Else) && arm.guard.is_none() {
                        if else_body.is_none() {
                            else_body = Some(arm.body);
                        }
                    } else {
                        arms.push(arm);
                    }
                }
                ParsedArm::Failed(input) => {
                    if recover {
                        recovered.push(input);
                    }
                }
            }
        }
        if recover {
            self.record_recovered_when(ast::RecoveredWhen {
                block: None,
                start: keyword.span.start,
                subject: subject.clone(),
                arms: recovered,
            });
        }
        let close = result?;
        Ok(When {
            subject,
            arms,
            else_body,
            span: Span::new(keyword.span.start, close.span.end),
        })
    }

    fn parse_when_arms(
        &mut self,
        has_subject: bool,
        parsed: &mut Vec<ParsedArm>,
    ) -> Result<Token, Diagnostic> {
        let body_depth = self.brace_depth();
        let mut has_else = false;
        let mut reported_after_else = false;
        loop {
            match self.peek().kind {
                TokenKind::RBrace => return Ok(self.bump()),
                TokenKind::Eof => return self.missing_token(TokenKind::RBrace, "`}`"),
                _ => {
                    let start = self.pos;
                    let offset = self.peek().span.start;
                    if has_else && !reported_after_else {
                        let diagnostic = self.unexpected::<()>("`}`").unwrap_err();
                        self.diagnostics.push(diagnostic);
                        reported_after_else = true;
                    }
                    let (result, mut recovery) = self.parse_when_arm(has_subject);
                    let span = Span::new(offset, self.peek().span.start);
                    if has_else {
                        recovery = Some(ast::RecoveredWhenArm::Unavailable(span));
                    }
                    match result {
                        Ok(arm) => {
                            has_else |= matches!(arm.condition, WhenArmCondition::Else)
                                && arm.guard.is_none();
                            parsed.push(ParsedArm::Projected {
                                arm,
                                recovery: recovery.map(Box::new),
                            });
                        }
                        Err(diagnostic) => {
                            parsed.push(ParsedArm::Failed(
                                recovery.unwrap_or(ast::RecoveredWhenArm::Unavailable(span)),
                            ));
                            if self.at_eof() {
                                return Err(diagnostic);
                            }
                            self.diagnostics.push(diagnostic);
                            self.synchronize_body_item(body_depth, start);
                        }
                    }
                }
            }
        }
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
