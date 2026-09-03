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
            let target = if prepared.len() == 1 {
                match prepared[0].target {
                    crate::call_resolution::candidates::CallableSource::Free(_) => {
                        format!("function `{name}`")
                    }
                    crate::call_resolution::candidates::CallableSource::Local { .. } => {
                        format!("local function `{name}`")
                    }
                    crate::call_resolution::candidates::CallableSource::Method(_) => {
                        format!("method `{name}`")
                    }
                }
            } else {
                format!("`{name}`")
            };
            self.error(
                span,
                format!(
                    "{target} takes exactly {uniform_arity} {noun}, but {supplied} were supplied"
                ),
            );
            return;
        }
        if prepared.len() == 1
            && extension_receiver.is_none()
            && prepared[0].explicit_arity_match
            && prepared[0].params.len() == arg_tys.len()
            && let Some(arg_exprs) = arg_exprs
        {
            let candidate = &prepared[0];
            let type_params = self.signatures[&candidate.function].type_params.clone();
            let mut bindings = candidate.initial_bindings.clone();
            for ((&parameter, argument), expression) in
                candidate.params.iter().zip(arg_tys).zip(arg_exprs)
            {
                let Some(argument) = *argument else {
                    continue;
                };
                if !self.bind_type_args(
                    parameter,
                    argument,
                    &mut bindings,
                    &type_params,
                    expression.span(),
                ) {
                    return;
                }
            }
            if let Some((index, parameter)) =
                bindings.iter().zip(&type_params).enumerate().find_map(
                    |(index, (binding, parameter))| binding.is_none().then_some((index, parameter)),
                )
            {
                debug_assert!(index < type_params.len());
                self.error(
                    span,
                    format!(
                        "cannot infer type argument `{}` for `{name}`",
                        parameter.name
                    ),
                );
                return;
            }
            let type_arguments: Vec<_> = bindings.into_iter().flatten().collect();
            let parameter_names: Vec<_> = self.signatures[&candidate.function]
                .params
                .iter()
                .map(|parameter| parameter.name.text.clone())
                .collect();
            for (((parameter, argument), expression), parameter_name) in candidate
                .params
                .iter()
                .zip(arg_tys)
                .zip(arg_exprs)
                .zip(&parameter_names)
            {
                let Some(argument) = *argument else {
                    continue;
                };
                let expected = self.substitute_call_level(*parameter, &type_arguments);
                if !self.is_subtype(argument, expected) {
                    self.error(
                        expression.span(),
                        format!(
                            "argument for parameter `{}` of `{name}` must be of type {}, found {}",
                            parameter_name,
                            self.type_name(expected),
                            self.type_name(argument)
                        ),
                    );
                    return;
                }
            }
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
