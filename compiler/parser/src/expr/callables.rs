//! Callable references, anonymous functions, and lambda literals.

use scoop_ast::{Diagnostic, Expr, LambdaParam, Param, Span};

use crate::lexer::TokenKind;
use crate::parser::Parser;

impl Parser {
    pub(super) fn parse_callable_reference(
        &mut self,
        receiver: Option<Expr>,
    ) -> Result<Expr, Diagnostic> {
        let start = receiver
            .as_ref()
            .map_or_else(|| self.peek().span.start, |expr| expr.span().start);
        self.expect("`::`", |kind| matches!(kind, TokenKind::DoubleColon))?;
        let name = self.expect_ident("callable name after `::`")?;
        let span = Span::new(start, name.span.end);
        Ok(Expr::CallableReference {
            id: self.alloc_callable_reference_id(),
            receiver: receiver.map(Box::new),
            name,
            span,
        })
    }

    pub(super) fn parse_anonymous_function(
        &mut self,
        is_suspend: bool,
        expression_start: Option<u32>,
    ) -> Result<Expr, Diagnostic> {
        let keyword = self.expect("`fun`", |kind| matches!(kind, TokenKind::Fun))?;
        self.expect("`(`", |kind| matches!(kind, TokenKind::LParen))?;
        let mut params = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RParen) {
            loop {
                let name = self.expect_ident("parameter name")?;
                self.expect("`:`", |kind| matches!(kind, TokenKind::Colon))?;
                let ty = self.parse_type_ref()?;
                params.push(Param {
                    span: Span::new(name.span.start, ty.span.end),
                    name,
                    ty,
                });
                if matches!(self.peek().kind, TokenKind::Comma) {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        self.expect("`)`", |kind| matches!(kind, TokenKind::RParen))?;
        let return_ty = if matches!(self.peek().kind, TokenKind::Colon) {
            self.bump();
            Some(self.parse_type_ref()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        Ok(Expr::AnonymousFunction {
            id: self.alloc_anonymous_function_id(),
            is_suspend,
            params,
            return_ty,
            span: Span::new(
                expression_start.unwrap_or(keyword.span.start),
                body.span.end,
            ),
            body,
        })
    }

    /// A lambda body. Parameter parsing is speculative only until a `->`
    /// is found; without it the same tokens are parsed as the first body
    /// statement, which preserves `{ value }` as a zero/implicit-parameter
    /// lambda rather than treating `value` as a declaration.
    pub(super) fn parse_lambda(
        &mut self,
        is_suspend: bool,
        expression_start: Option<u32>,
    ) -> Result<Expr, Diagnostic> {
        let open = self.expect("`{`", |kind| matches!(kind, TokenKind::LBrace))?;
        let header_start = self.pos;
        let diagnostics_start = self.diagnostics.len();
        let parameters = if matches!(self.peek().kind, TokenKind::Arrow) {
            self.bump();
            Some(Vec::new())
        } else {
            let parsed = self.try_parse_lambda_parameters();
            match parsed {
                Some(parameters) => Some(parameters),
                None => {
                    self.pos = header_start;
                    self.diagnostics.truncate(diagnostics_start);
                    None
                }
            }
        };
        let body = self.parse_block_after_open(open.clone())?;
        let span = Span::new(expression_start.unwrap_or(open.span.start), body.span.end);
        Ok(Expr::Lambda {
            id: self.alloc_lambda_id(),
            is_suspend,
            parameters,
            body,
            span,
        })
    }

    fn try_parse_lambda_parameters(&mut self) -> Option<Vec<LambdaParam>> {
        let start = self.pos;
        let mut parameters = Vec::new();
        loop {
            let target = self.parse_pattern().ok()?;
            let target_span = crate::pattern::pattern_span(&target);
            let ty = if matches!(self.peek().kind, TokenKind::Colon) {
                self.bump();
                Some(self.parse_type_ref().ok()?)
            } else {
                None
            };
            let end = ty.as_ref().map_or(target_span.end, |ty| ty.span.end);
            parameters.push(LambdaParam {
                target,
                ty,
                span: Span::new(target_span.start, end),
            });
            if matches!(self.peek().kind, TokenKind::Comma) {
                self.bump();
                continue;
            }
            if matches!(self.peek().kind, TokenKind::Arrow) {
                self.bump();
                return Some(parameters);
            }
            self.pos = start;
            return None;
        }
    }
}
