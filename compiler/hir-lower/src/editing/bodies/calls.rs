use super::*;

mod arguments;
mod delegations;

#[derive(Clone)]
pub(in crate::editing) struct CallInput {
    pub(in crate::editing) callee_end: u32,
    pub(in crate::editing) callee: Option<ast::Span>,
    pub(in crate::editing) name: Option<String>,
    pub(in crate::editing) arguments: Vec<(ast::Span, bool, Option<ast::Span>)>,
}

#[derive(Clone)]
pub(super) struct CheckedArguments {
    parameters: Vec<hir::EditingAppliedParameter>,
    arguments: Vec<hir::EditingCallArgument>,
}

impl DeclarationAnalysis {
    pub fn calls(
        &mut self,
        body: impl Into<hir::EditingSourceBodyId>,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<&[hir::EditingAppliedCall]>, EditingCancelled> {
        let body = body.into();
        self.source_body_facts(body, is_cancelled)?;
        Ok(match &self.body_facts[&body] {
            V::Known(body) => V::Known(&body.calls),
            V::Unavailable(reason) => V::Unavailable(*reason),
        })
    }
}

impl Lowerer {
    pub(crate) fn observe_callable_syntax(&mut self, expression: &ast::Expr) {
        if !self.observes_current_body() {
            return;
        }
        if let ast::Expr::Binary {
            operator_span: Some(operator),
            span,
            ..
        }
        | ast::Expr::Unary {
            operator_span: Some(operator),
            span,
            ..
        } = expression
        {
            self.editing_body
                .as_mut()
                .expect("active query")
                .callable_names
                .insert(*span, *operator);
            return;
        }
        if let ast::Expr::CallableReference { name, span, .. } = expression {
            let names = &mut self
                .editing_body
                .as_mut()
                .expect("active query")
                .callable_names;
            names.insert(*span, name.span);
            names.insert(name.span, name.span);
            return;
        }
        let Some(input) = CallInput::from_expression(expression) else {
            return;
        };
        let body = self.editing_body.as_mut().expect("active query");
        if let Some(span) = input.callee {
            body.callable_names.insert(span, span);
        }
        if body.failed || self.diagnostics.len() != body.diagnostics_start {
            return;
        }
        body.call_inputs.insert(expression.span(), input);
    }

    pub(crate) fn observe_callable_reference(
        &mut self,
        span: ast::Span,
        target: hir::SemanticCallable,
    ) {
        self.observe_callee_reference(span, target.into());
    }

    pub(crate) fn observe_callee_reference(&mut self, span: ast::Span, target: hir::EditingCallee) {
        if !self.observes_current_body() {
            return;
        }
        let body = self.editing_body.as_ref().expect("active query");
        let Some(span) = body.callable_names.get(&span).copied().or_else(|| {
            if matches!(target, hir::EditingCallee::Callable(_)) {
                return None;
            }
            body.call_inputs
                .get(&span)
                .and_then(|input| input.callee)
                .or_else(|| body.property_names.get(&span).copied())
        }) else {
            return;
        };
        let definition = self.editing_callee_origin(target);
        self.editing_body
            .as_mut()
            .expect("active query")
            .callables
            .insert(
                span,
                hir::EditingCallableReference {
                    span,
                    target,
                    definition,
                },
            );
    }

    pub(in crate::editing) fn observe_completed_call(
        &mut self,
        span: ast::Span,
        result: hir::TypeId,
        context: hir::SourceContextId,
    ) {
        let body = self.editing_body.as_mut().expect("active body query");
        if body.failed || self.diagnostics.len() != body.diagnostics_start {
            return;
        }
        let Some(arguments) = body.argument_maps.remove(&span) else {
            return;
        };
        let input = &body.call_inputs[&span];
        let callee = input
            .callee
            .and_then(|span| body.callables.get(&span))
            .map(|reference| reference.target);
        body.calls.insert(
            span,
            hir::EditingAppliedCall {
                span,
                callee_span: input.callee,
                callee_end: input.callee_end,
                name: input.name.clone(),
                callee,
                context: hir::EditingCallContext::Body {
                    body: body.id,
                    source: context,
                },
                parameters: arguments.parameters,
                arguments: arguments.arguments,
                result,
            },
        );
    }
}

impl CallInput {
    pub(in crate::editing) fn from_expression(expression: &ast::Expr) -> Option<Self> {
        if let ast::Expr::InfixCall {
            target: ast::InfixTarget::Named(name),
            rhs,
            ..
        } = expression
        {
            return Some(Self {
                callee_end: name.span.end,
                callee: Some(name.span),
                name: Some(name.text.clone()),
                arguments: vec![(rhs.span(), false, None)],
            });
        }
        let (name, callee_end, args) = match expression {
            ast::Expr::Call(call) => (Some(&call.callee), call.callee.span.end, &call.args),
            ast::Expr::MethodCall { name, args, .. }
            | ast::Expr::SuperMethodCall { name, args, .. }
            | ast::Expr::QualifiedInterfaceSuperMethodCall { name, args, .. } => {
                (Some(name), name.span.end, args)
            }
            ast::Expr::Invoke { callee, args, .. } => (None, callee.span().end, args),
            _ => return None,
        };
        Some(Self::from_arguments(name, callee_end, args))
    }

    pub(in crate::editing) fn from_arguments(
        name: Option<&ast::Ident>,
        callee_end: u32,
        args: &[ast::CallArgument],
    ) -> Self {
        Self {
            callee_end,
            callee: name.map(|name| name.span),
            name: name.map(|name| name.text.clone()),
            arguments: args
                .iter()
                .map(|argument| {
                    (
                        argument.span,
                        matches!(argument.name, ast::CallArgumentName::TrailingLambda),
                        match &argument.name {
                            ast::CallArgumentName::Named(name) => Some(name.span),
                            _ => None,
                        },
                    )
                })
                .collect(),
        }
    }
}
