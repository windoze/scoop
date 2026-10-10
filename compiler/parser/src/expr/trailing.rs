use super::*;

impl Parser {
    pub(super) fn starts_trailing_lambda(&self) -> bool {
        if self.at_arm_body_newline() {
            return false;
        }
        matches!(self.peek().kind, TokenKind::LBrace)
            || (matches!(self.peek().kind, TokenKind::Suspend)
                && self
                    .tokens
                    .get(self.pos + 1)
                    .is_some_and(|next| matches!(next.kind, TokenKind::LBrace)))
    }

    pub(super) fn parse_call_inputs(&mut self) -> Result<(Vec<CallArgument>, u32), Diagnostic> {
        if matches!(self.peek().kind, TokenKind::LParen) {
            self.parse_args()
        } else if self.starts_trailing_lambda() {
            Ok((Vec::new(), self.tokens[self.pos - 1].span.end))
        } else {
            self.unexpected("call arguments")
        }
    }

    pub(super) fn parse_trailing_lambda(
        &mut self,
        receiver: Expr,
        grouped: bool,
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
        let (args, span) = match &mut call {
            Expr::Call(call) => (&mut call.args, &mut call.span),
            Expr::MethodCall { args, span, .. }
            | Expr::SuperMethodCall { args, span, .. }
            | Expr::QualifiedInterfaceSuperMethodCall { args, span, .. }
            | Expr::Invoke { args, span, .. } => (args, span),
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
        let suspend =
            matches!(self.peek().kind, TokenKind::Suspend).then(|| self.bump().span.start);
        let expression = self.parse_lambda(suspend.is_some(), suspend)?;
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
