use super::*;
use scoop_ast::{
    MemberRequirement, MemberRequirements, RequiredCall, RequirementArgument,
    RequirementArgumentValue, RequirementParameter, SpreadSyntax,
};

impl Parser {
    pub(super) fn parse_member_requirements(&mut self) -> Result<MemberRequirements, Diagnostic> {
        let start = self.bump().span.start;
        let mut parameters = Vec::new();
        if matches!(self.peek().kind, TokenKind::LParen) {
            self.bump();
            while !matches!(self.peek().kind, TokenKind::RParen) {
                let name = self.expect_ident("virtual parameter name")?;
                self.expect("`:`", |kind| matches!(kind, TokenKind::Colon))?;
                let ty = self.parse_type_ref()?;
                parameters.push(RequirementParameter { name, ty });
                if !matches!(self.peek().kind, TokenKind::Comma) {
                    break;
                }
                self.bump();
            }
            self.expect("`)`", |kind| matches!(kind, TokenKind::RParen))?;
        }
        self.expect("`{` after requires parameters", |kind| {
            matches!(kind, TokenKind::LBrace)
        })?;
        let mut calls = Vec::new();
        while !matches!(self.peek().kind, TokenKind::RBrace) {
            calls.push(self.parse_member_requirement()?);
            if matches!(self.peek().kind, TokenKind::Semicolon) {
                while matches!(self.peek().kind, TokenKind::Semicolon) {
                    self.bump();
                }
            } else if !self.peek().newline_before && !matches!(self.peek().kind, TokenKind::RBrace)
            {
                return self.unexpected("newline or `;` between member requirements");
            }
        }
        let end = self.bump().span.end;
        if calls.is_empty() {
            return Err(Diagnostic::at(
                Span::new(start, end),
                "a requires block must contain at least one call",
            ));
        }
        Ok(MemberRequirements {
            parameters,
            calls,
            span: Span::new(start, end),
        })
    }

    fn parse_member_requirement(&mut self) -> Result<MemberRequirement, Diagnostic> {
        let start = self.peek().span.start;
        let mut annotations = Vec::new();
        let mut suspend_span = None;
        loop {
            match self.peek().kind {
                TokenKind::At => annotations.extend(self.parse_annotations()?),
                TokenKind::Suspend => {
                    let span = self.bump().span;
                    if suspend_span.replace(span).is_some() {
                        return Err(Diagnostic::at(
                            span,
                            "duplicate `suspend` modifier on member requirement",
                        ));
                    }
                }
                _ => break,
            }
        }
        let receiver = self.expect_ident("requirement receiver or type parameter")?;
        let method = if matches!(self.peek().kind, TokenKind::Dot) {
            self.bump();
            let mut name = self.expect_ident("required method name")?;
            let companion = name.text == "Companion" && matches!(self.peek().kind, TokenKind::Dot);
            if companion {
                self.bump();
                name = self.expect_ident("required companion method name")?;
            }
            let type_arguments = self.parse_explicit_call_type_args()?;
            Some((name, companion, type_arguments))
        } else {
            None
        };
        let arguments = self.parse_requirement_arguments()?;
        let result = if matches!(self.peek().kind, TokenKind::Colon) {
            self.bump();
            Some(self.parse_type_ref()?)
        } else {
            None
        };
        let call = if let Some((name, companion, type_arguments)) = method {
            let Some(result) = result else {
                return Err(Diagnostic::at(
                    self.tokens[self.pos - 1].span,
                    "a required method call must declare its result type",
                ));
            };
            RequiredCall::Method {
                receiver,
                companion,
                name,
                type_arguments,
                result,
            }
        } else {
            RequiredCall::Constructor {
                parameter: receiver,
                result,
            }
        };
        Ok(MemberRequirement {
            annotations,
            suspend_span,
            call,
            arguments,
            span: Span::new(start, self.tokens[self.pos - 1].span.end),
        })
    }

    fn parse_requirement_arguments(&mut self) -> Result<Vec<RequirementArgument>, Diagnostic> {
        self.expect("`(` in member requirement", |kind| {
            matches!(kind, TokenKind::LParen)
        })?;
        let mut arguments = Vec::new();
        while !matches!(self.peek().kind, TokenKind::RParen) {
            let start = self.peek().span.start;
            let name = if matches!(
                self.peek().kind,
                TokenKind::Ident(_) | TokenKind::EscapedIdent(_)
            ) && matches!(self.tokens[self.pos + 1].kind, TokenKind::Equal)
            {
                let name = self.expect_ident("required argument name")?;
                self.bump();
                Some(name)
            } else {
                None
            };
            let spread = if matches!(self.peek().kind, TokenKind::Star) {
                SpreadSyntax::Spread(self.bump().span)
            } else {
                SpreadSyntax::Plain
            };
            let argument = self.expect_ident("virtual argument or inline typed argument")?;
            let value = if matches!(self.peek().kind, TokenKind::Colon) {
                self.bump();
                RequirementArgumentValue::Inline {
                    name: argument,
                    ty: self.parse_type_ref()?,
                }
            } else {
                RequirementArgumentValue::Parameter(argument)
            };
            arguments.push(RequirementArgument {
                name,
                spread,
                value,
                span: Span::new(start, self.tokens[self.pos - 1].span.end),
            });
            if !matches!(self.peek().kind, TokenKind::Comma) {
                break;
            }
            self.bump();
        }
        self.expect("`)`", |kind| matches!(kind, TokenKind::RParen))?;
        Ok(arguments)
    }
}
