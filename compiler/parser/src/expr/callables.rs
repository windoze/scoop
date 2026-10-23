//! Callable references, anonymous functions, and lambda literals.

use scoop_ast::{Diagnostic, Expr, LambdaParam, Param, Span};

use crate::lexer::TokenKind;
use crate::parser::Parser;

mod parameters;

impl Parser {
    pub(crate) fn starts_anonymous_function(&mut self) -> bool {
        let saved = self.checkpoint();
        if matches!(self.peek().kind, TokenKind::Suspend) {
            self.bump();
        }
        self.bump(); // `fun`
        let ordinary = matches!(self.peek().kind, TokenKind::LParen);
        let receiver = self.parse_type_atom_with_suffix().is_ok()
            && matches!(self.peek().kind, TokenKind::Dot | TokenKind::QuestionDot);
        let anonymous = if receiver && !self.has_errors_since(saved) {
            matches!(self.tokens[self.pos + 1].kind, TokenKind::LParen)
        } else {
            ordinary
        };
        self.restore(saved);
        anonymous
    }

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
        self.with_node(scoop_syntax::SyntaxKind::AnonymousFunctionExpr, |parser| {
            parser.parse_anonymous_function_inner(is_suspend, expression_start)
        })
    }

    fn parse_anonymous_function_inner(
        &mut self,
        is_suspend: bool,
        expression_start: Option<u32>,
    ) -> Result<Expr, Diagnostic> {
        let checkpoint = self.checkpoint();
        let keyword = self.expect("`fun`", |kind| matches!(kind, TokenKind::Fun))?;
        let receiver_start = self.checkpoint();
        let receiver_annotations = self.parse_annotations()?;
        let receiver = match self.parse_type_atom_with_suffix() {
            Ok(receiver) if matches!(self.peek().kind, TokenKind::Dot | TokenKind::QuestionDot) => {
                Some(self.parse_receiver_separator(receiver))
            }
            _ => {
                self.restore(receiver_start);
                None
            }
        }
        .map(|receiver| receiver.with_annotations(receiver_annotations));
        let params = self.parse_anonymous_parameters()?;
        let return_ty = if matches!(self.peek().kind, TokenKind::Colon) {
            self.bump();
            Some(self.parse_type_ref()?)
        } else {
            None
        };
        let complete_header = self.fragment_is_complete(checkpoint);
        let opening = matches!(self.peek().kind, TokenKind::LBrace)
            .then_some(scoop_ast::BodyBlockId(self.peek().span.start));
        let body_checkpoint = self.checkpoint();
        let result = self.parse_block_with_scope(scoop_ast::RecoveredBlockScope::AnonymousFunction);
        if complete_header
            && let Some(body) = opening
            && (result.is_err() || !self.fragment_is_complete(body_checkpoint))
        {
            self.record_recovered_callable(scoop_ast::RecoveredCallableLiteral {
                block: None,
                span: Span::new(
                    expression_start.unwrap_or(keyword.span.start),
                    result
                        .as_ref()
                        .map_or(self.peek().span.start, |body| body.span.end),
                ),
                label: None,
                is_suspend,
                header: scoop_ast::RecoveredCallableHeader::AnonymousFunction {
                    receiver: receiver.clone().map(Box::new),
                    parameters: params.clone(),
                    return_ty: return_ty.clone().map(Box::new),
                },
                body,
            });
        }
        let body = result?;
        Ok(Expr::AnonymousFunction {
            label: None,
            receiver: receiver.map(Box::new),
            id: self.alloc_anonymous_function_id(),
            is_suspend,
            params,
            return_ty: return_ty.map(Box::new),
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
        self.with_node(scoop_syntax::SyntaxKind::LambdaExpr, |parser| {
            parser.parse_lambda_inner(is_suspend, expression_start)
        })
    }

    fn parse_lambda_inner(
        &mut self,
        is_suspend: bool,
        expression_start: Option<u32>,
    ) -> Result<Expr, Diagnostic> {
        let checkpoint = self.checkpoint();
        let open = self.expect("`{`", |kind| matches!(kind, TokenKind::LBrace))?;
        let header_start = self.checkpoint();
        let parameters = if matches!(self.peek().kind, TokenKind::Arrow) {
            self.bump();
            Some(Vec::new())
        } else {
            let parsed = self.try_parse_lambda_parameters();
            match parsed {
                Some(parameters) => Some(parameters),
                None => {
                    self.restore(header_start);
                    None
                }
            }
        };
        let complete_header = self.fragment_is_complete(checkpoint);
        let body_checkpoint = self.checkpoint();
        let result =
            self.parse_block_after_open(open.clone(), scoop_ast::RecoveredBlockScope::Lambda);
        if complete_header && (result.is_err() || !self.fragment_is_complete(body_checkpoint)) {
            self.record_recovered_callable(scoop_ast::RecoveredCallableLiteral {
                block: None,
                span: Span::new(
                    expression_start.unwrap_or(open.span.start),
                    result
                        .as_ref()
                        .map_or(self.peek().span.start, |body| body.span.end),
                ),
                label: None,
                is_suspend,
                header: scoop_ast::RecoveredCallableHeader::Lambda {
                    parameters: parameters.clone(),
                },
                body: scoop_ast::BodyBlockId(open.span.start),
            });
        }
        let body = result?;
        let span = Span::new(expression_start.unwrap_or(open.span.start), body.span.end);
        Ok(Expr::Lambda {
            label: None,
            id: self.alloc_lambda_id(),
            is_suspend,
            parameters,
            body,
            span,
        })
    }

    fn try_parse_lambda_parameters(&mut self) -> Option<Vec<LambdaParam>> {
        self.with_node(scoop_syntax::SyntaxKind::ParameterList, |parser| {
            let parsed =
                parser.parse_delimited_items(crate::parser::ListKind::LambdaParameters, |parser| {
                    parser.with_node(
                        scoop_syntax::SyntaxKind::Parameter,
                        Self::parse_lambda_parameter,
                    )
                });
            parsed.closed.then_some(parsed.items)
        })
    }
}
