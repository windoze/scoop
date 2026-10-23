use super::*;
use scoop_ast as ast;

impl Parser {
    pub(super) fn starts_trailing_lambda(&self) -> bool {
        if self.at_arm_body_newline()
            || self.supertype_expression_nesting == Some(self.expression_nesting[self.pos])
        {
            return false;
        }
        let offset = if self.starts_callable_label() { 2 } else { 0 };
        self.tokens.get(self.pos + offset).is_some_and(|token| {
            matches!(token.kind, TokenKind::LBrace)
                || (matches!(token.kind, TokenKind::Suspend)
                    && self
                        .tokens
                        .get(self.pos + offset + 1)
                        .is_some_and(|next| matches!(next.kind, TokenKind::LBrace)))
        })
    }

    pub(super) fn parse_call_inputs(&mut self) -> Result<ParsedArguments, Diagnostic> {
        if matches!(self.peek().kind, TokenKind::LParen) {
            self.parse_arguments()
        } else if self.starts_trailing_lambda() {
            Ok(ParsedArguments {
                args: Vec::new(),
                end: self.tokens[self.pos - 1].span.end,
                recovery: None,
                complete: true,
            })
        } else {
            self.unexpected("call arguments")
        }
    }

    pub(super) fn finish_recovered_call(
        &mut self,
        expression: Expr,
        recovery: Option<ast::RecoveredCallArguments>,
        target_complete: bool,
        arguments_complete: bool,
    ) -> Result<Expr, Diagnostic> {
        if self.starts_trailing_lambda() {
            return self.parse_trailing_lambda(
                expression,
                false,
                target_complete,
                arguments_complete,
                recovery,
            );
        }
        if recovery.is_some() {
            self.record_recovered_call(&expression, recovery, target_complete, None);
        }
        Ok(expression)
    }

    pub(super) fn parse_trailing_lambda(
        &mut self,
        receiver: Expr,
        grouped: bool,
        target_complete: bool,
        arguments_complete: bool,
        recovery: Option<ast::RecoveredCallArguments>,
    ) -> Result<Expr, Diagnostic> {
        let mut call = if grouped {
            invoke(receiver)
        } else {
            match receiver {
                Expr::Call(_)
                | Expr::MethodCall { .. }
                | Expr::SuperMethodCall { .. }
                | Expr::QualifiedInterfaceSuperMethodCall { .. }
                | Expr::Invoke { .. } => receiver,
                Expr::Var(callee) => Expr::Call(CallExpr {
                    span: callee.span,
                    callee,
                    type_args: Vec::new(),
                    args: Vec::new(),
                }),
                other => invoke(other),
            }
        };
        let argument_label = match &call {
            Expr::Call(call) => Some(call.callee.clone()),
            Expr::MethodCall { name, .. }
            | Expr::SuperMethodCall { name, .. }
            | Expr::QualifiedInterfaceSuperMethodCall { name, .. } => Some(name.clone()),
            _ => None,
        };
        let args = match &call {
            Expr::Call(call) => &call.args,
            Expr::MethodCall { args, .. }
            | Expr::SuperMethodCall { args, .. }
            | Expr::QualifiedInterfaceSuperMethodCall { args, .. }
            | Expr::Invoke { args, .. } => args,
            _ => unreachable!("a trailing lambda constructs a call"),
        };
        if args
            .iter()
            .any(|argument| matches!(argument.name, CallArgumentName::TrailingLambda))
        {
            return Err(Diagnostic::at(
                self.peek().span,
                "a call accepts only one trailing lambda; group the call to invoke its result",
            ));
        }
        let checkpoint = self.checkpoint();
        let start = self.peek().span.start;
        let mut expression = self.parse_atom();
        if let (Some(label), Ok(expression)) = (&argument_label, &mut expression) {
            labels::name_lambda(expression, label);
        }
        let literal_complete = expression.is_ok() && self.fragment_is_complete(checkpoint);
        if !target_complete || !arguments_complete || !literal_complete {
            let end = expression
                .as_ref()
                .map_or(self.peek().span.start, |value| value.span().end);
            let span = Span::new(start, end);
            let value = match &expression {
                Ok(expression) if literal_complete => {
                    ast::RecoveredArgumentValue::Complete(Box::new(expression.clone()))
                }
                _ => ast::RecoveredArgumentValue::Unavailable(span),
            };
            self.record_recovered_call(
                &call,
                recovery,
                target_complete,
                Some(ast::RecoveredCallArgument {
                    name: CallArgumentName::TrailingLambda,
                    spread: SpreadSyntax::Plain,
                    span,
                    value,
                }),
            );
        }
        let expression = expression?;
        let (args, span) = match &mut call {
            Expr::Call(call) => (&mut call.args, &mut call.span),
            Expr::MethodCall { args, span, .. }
            | Expr::SuperMethodCall { args, span, .. }
            | Expr::QualifiedInterfaceSuperMethodCall { args, span, .. }
            | Expr::Invoke { args, span, .. } => (args, span),
            _ => unreachable!("a trailing lambda constructs a call"),
        };
        let argument_span = expression.span();
        span.end = argument_span.end;
        args.push(CallArgument {
            name: CallArgumentName::TrailingLambda,
            spread: SpreadSyntax::Plain,
            expression,
            span: argument_span,
        });
        Ok(call)
    }
}

fn invoke(callee: Expr) -> Expr {
    Expr::Invoke {
        span: callee.span(),
        callee: Box::new(callee),
        type_args: Vec::new(),
        args: Vec::new(),
    }
}
