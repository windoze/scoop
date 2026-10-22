use super::*;

mod arguments;

#[derive(Clone)]
pub(super) struct CallInput {
    callee_end: u32,
    name: Option<String>,
    arguments: Vec<(ast::Span, bool)>,
}

#[derive(Clone)]
pub(super) struct CheckedArguments {
    parameters: Vec<hir::EditingAppliedParameter>,
    arguments: Vec<hir::EditingCallArgument>,
}

impl DeclarationAnalysis {
    pub fn calls(
        &mut self,
        function: hir::FunctionId,
        is_cancelled: impl Fn() -> bool + 'static,
    ) -> Result<V<&[hir::EditingAppliedCall]>, EditingCancelled> {
        self.function_facts(function, is_cancelled)?;
        Ok(match &self.body_facts[&function] {
            V::Known(body) => V::Known(&body.calls),
            V::Unavailable(reason) => V::Unavailable(*reason),
        })
    }
}

impl Lowerer {
    pub(crate) fn observe_call_syntax(&mut self, expression: &ast::Expr) {
        if !self.observes_current_body() {
            return;
        }
        let (name, callee_end, args) = match expression {
            ast::Expr::Call(call) => (Some(&call.callee), call.callee.span.end, &call.args),
            ast::Expr::MethodCall { name, args, .. }
            | ast::Expr::SuperMethodCall { name, args, .. }
            | ast::Expr::QualifiedInterfaceSuperMethodCall { name, args, .. } => {
                (Some(name), name.span.end, args)
            }
            ast::Expr::Invoke { callee, args, .. } => (None, callee.span().end, args),
            _ => return,
        };
        let body = self.editing_body.as_mut().expect("active query");
        if let Some(name) = name {
            body.call_names.insert(name.span);
        }
        if body.failed || self.diagnostics.len() != body.diagnostics_start {
            return;
        }
        body.call_inputs.insert(
            expression.span(),
            CallInput {
                callee_end,
                name: name.map(|name| name.text.clone()),
                arguments: args
                    .iter()
                    .map(|argument| {
                        (
                            argument.span,
                            matches!(argument.name, ast::CallArgumentName::TrailingLambda),
                        )
                    })
                    .collect(),
            },
        );
    }

    pub(crate) fn observe_callable_reference(
        &mut self,
        span: ast::Span,
        target: hir::SemanticCallable,
    ) {
        if !self.observes_current_body()
            || !self
                .editing_body
                .as_ref()
                .expect("active query")
                .call_names
                .contains(&span)
        {
            return;
        }
        let definition = self.editing_callable_origin(target);
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

    pub(super) fn observe_completed_call(
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
        body.calls.insert(
            span,
            hir::EditingAppliedCall {
                span,
                callee_end: input.callee_end,
                name: input.name.clone(),
                context,
                parameters: arguments.parameters,
                arguments: arguments.arguments,
                result,
            },
        );
    }
}
