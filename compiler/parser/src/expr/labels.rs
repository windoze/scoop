use super::*;

impl Parser {
    pub(super) fn starts_callable_label(&self) -> bool {
        matches!(
            self.peek().kind,
            TokenKind::Ident(_) | TokenKind::EscapedIdent(_)
        ) && self
            .tokens
            .get(self.pos + 1)
            .is_some_and(|token| !token.newline_before && matches!(token.kind, TokenKind::At))
    }

    pub(super) fn parse_labeled_callable(&mut self) -> Result<Expr, Diagnostic> {
        let label = self.expect_ident("callable label")?;
        self.expect("`@`", |kind| matches!(kind, TokenKind::At))?;
        if !matches!(
            self.peek().kind,
            TokenKind::Fun | TokenKind::Suspend | TokenKind::LBrace
        ) {
            return self.unexpected("lambda or anonymous function after label");
        }
        let start = self.peek().span.start;
        let checkpoint = self.checkpoint();
        let result = self.parse_atom();
        if result.is_err() || !self.fragment_is_complete(checkpoint) {
            self.label_recovered_callable(start, label.clone());
        }
        let mut expression = result?;
        match &mut expression {
            Expr::Lambda {
                label: target,
                span,
                ..
            }
            | Expr::AnonymousFunction {
                label: target,
                span,
                ..
            } => {
                span.start = label.span.start;
                *target = Some(label);
            }
            _ => return self.unexpected("lambda or anonymous function after label"),
        }
        Ok(expression)
    }
}

pub(super) fn name_call_lambdas(expression: &mut Expr) {
    let (name, args) = match expression {
        Expr::Call(call) => (&call.callee, &mut call.args),
        Expr::MethodCall { name, args, .. }
        | Expr::SuperMethodCall { name, args, .. }
        | Expr::QualifiedInterfaceSuperMethodCall { name, args, .. } => (&*name, args),
        _ => return,
    };
    for argument in args {
        name_lambda(&mut argument.expression, name);
    }
}

pub(crate) fn name_lambda(expression: &mut Expr, name: &Ident) {
    if let Expr::Lambda { label, .. } = expression
        && label.is_none()
    {
        *label = Some(name.clone());
    }
}
