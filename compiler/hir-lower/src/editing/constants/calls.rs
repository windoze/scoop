use super::*;
use hir::{EditingUnavailable as U, EditingValue as V};

impl DeclarationAnalysis {
    pub fn static_calls(
        &self,
        source: &scoop_identity::SourceIdentity,
        is_cancelled: impl Fn() -> bool,
    ) -> Result<V<Vec<hir::EditingAppliedCall>>, EditingCancelled> {
        if is_cancelled() {
            return Err(EditingCancelled);
        }
        if !self.declarations_ready {
            return Ok(V::Unavailable(U::DeclarationFailure));
        }
        let mut calls = Vec::new();
        for call in &self.static_calls {
            if is_cancelled() {
                return Err(EditingCancelled);
            }
            if &call.source == source {
                calls.push(call.call.clone());
            }
        }
        Ok(V::Known(calls))
    }

    pub fn static_callable_reference(
        &self,
        source: &scoop_identity::SourceIdentity,
        span: ast::Span,
        is_cancelled: impl Fn() -> bool,
    ) -> Result<V<Option<hir::EditingCallableReference>>, EditingCancelled> {
        let calls = match self.static_calls(source, is_cancelled)? {
            V::Known(calls) => calls,
            V::Unavailable(reason) => return Ok(V::Unavailable(reason)),
        };
        Ok(V::Known(calls.iter().find_map(|call| {
            let target = call.callee.filter(|_| call.callee_span == Some(span))?;
            Some(hir::EditingCallableReference {
                span,
                target,
                definition: self.lowerer.editing_callee_origin(target),
            })
        })))
    }

    pub fn static_named_argument(
        &self,
        source: &scoop_identity::SourceIdentity,
        span: ast::Span,
        is_cancelled: impl Fn() -> bool,
    ) -> Result<V<Option<hir::EditingNamedArgument>>, EditingCancelled> {
        let calls = match self.static_calls(source, is_cancelled)? {
            V::Known(calls) => calls,
            V::Unavailable(reason) => return Ok(V::Unavailable(reason)),
        };
        Ok(V::Known(self.lowerer.editing_named_argument(&calls, span)))
    }
}

impl Lowerer {
    pub(crate) fn observe_constant_call_syntax(&mut self, expression: &ast::Expr) {
        let Some(frame) = self.constant_frame() else {
            return;
        };
        if let Some(input) = super::super::bodies::CallInput::from_expression(expression) {
            frame.call_inputs.insert(expression.span(), input);
        }
    }

    pub(crate) fn observe_constant_intrinsic(
        &mut self,
        span: ast::Span,
        callable: hir::SemanticCallable,
        parameters: &[String],
        parameter_type: Option<hir::TypeId>,
        result: hir::TypeId,
    ) {
        let Some(frame) = self.constant_frame() else {
            return;
        };
        let Some(input) = frame.call_inputs.get(&span) else {
            return;
        };
        let call = hir::EditingAppliedCall {
            span,
            callee_span: input.callee,
            callee_end: input.callee_end,
            name: input.name.clone(),
            callee: Some(callable.into()),
            context: hir::EditingCallContext::Declaration(hir::SemanticDeclarationBinders {
                owner: Vec::new(),
                callable: Vec::new(),
            }),
            parameters: parameters
                .iter()
                .map(|name| hir::EditingAppliedParameter {
                    name: Some(name.clone()),
                    value_type: parameter_type.expect("the checked intrinsic parameter type"),
                    vararg_element_type: None,
                })
                .collect(),
            arguments: input
                .arguments
                .iter()
                .enumerate()
                .map(
                    |(parameter, &(span, trailing, name_span))| hir::EditingCallArgument {
                        span,
                        name_span,
                        parameter,
                        trailing,
                    },
                )
                .collect(),
            result,
        };
        frame.calls.insert(span, call);
    }
}
