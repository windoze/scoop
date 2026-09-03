use super::*;

impl Lowerer {
    pub(super) fn contextual_no_applicable_diagnostic(
        &mut self,
        name: &str,
        arg_exprs: &[ast::Expr],
        failures: &[(usize, TypeId, String)],
    ) -> bool {
        let Some(argument) = failures.iter().map(|failure| failure.0).min() else {
            return false;
        };
        let mut details = Vec::new();
        for (_, expected, reason) in failures.iter().filter(|failure| failure.0 == argument) {
            let detail = format!("{}: {reason}", self.type_name(*expected));
            if !details.contains(&detail) {
                details.push(detail);
            }
        }
        self.error(
            arg_exprs[argument].span(),
            format!(
                "{} does not match any overload of `{name}`; candidate expectations: {}",
                contextual_expr_name(&arg_exprs[argument]),
                details.join("; ")
            ),
        );
        true
    }
    /// No applicable candidate: when every overload shares one arity
    /// and the call supplies a different count, report it as an arity
    /// error against the name (the shape arity diagnostics had before
    /// overloading — this keeps the `print` / `println` arity messages
    /// intact); otherwise report the unmatched argument types.
    pub(super) fn no_applicable_diagnostic(
        &mut self,
        name: &str,
        prepared: &[Candidate],
        arg_tys: &[Option<TypeId>],
        arg_exprs: Option<&[ast::Expr]>,
        extension_receiver: Option<TypeId>,
        span: Span,
    ) {
        let hidden_argument_count = usize::from(extension_receiver.is_some());
        let supplied = arg_tys.len();
        let uniform_arity = prepared[0].params.len() - hidden_argument_count;
        if prepared
            .iter()
            .all(|candidate| candidate.params.len() - hidden_argument_count == uniform_arity)
            && uniform_arity != supplied
        {
            let noun = if uniform_arity == 1 {
                "argument"
            } else {
                "arguments"
            };
            self.error(
                span,
                format!(
                    "`{name}` takes exactly {uniform_arity} {noun}, but {supplied} were supplied"
                ),
            );
            return;
        }
        let found: Vec<String> = arg_tys
            .iter()
            .enumerate()
            .map(|(index, ty)| match ty {
                Some(ty) => self.type_name(*ty),
                None => contextual_expr_name(
                    &arg_exprs.expect("only source arguments can remain contextual")[index],
                ),
            })
            .collect();
        let message = match extension_receiver {
            Some(receiver) => format!(
                "no overload of extension `{name}` matches receiver type {} and argument types ({})",
                self.type_name(receiver),
                found.join(", ")
            ),
            None => format!(
                "no overload of `{name}` matches argument types ({})",
                found.join(", ")
            ),
        };
        self.error(span, message);
    }
}

fn contextual_expr_name(expr: &ast::Expr) -> String {
    match expr {
        ast::Expr::Var(name) if name.text == "None" => "None".to_string(),
        ast::Expr::ArrayLiteral { elements, .. } if elements.is_empty() => "[]".to_string(),
        ast::Expr::Lambda { .. } => "lambda".to_string(),
        ast::Expr::AnonymousFunction { .. } => "anonymous function".to_string(),
        ast::Expr::CallableReference { .. } => "callable reference".to_string(),
        _ => "context-dependent expression".to_string(),
    }
}
