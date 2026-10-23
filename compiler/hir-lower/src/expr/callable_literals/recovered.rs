use super::*;
use ast::RecoveredCallableHeader;

impl Lowerer {
    pub(crate) fn with_recovered_callable_context<R>(
        &mut self,
        literal: &ast::RecoveredCallableLiteral,
        expected: Option<ExpressionExpectation>,
        query: impl FnOnce(&mut Self) -> R,
    ) -> Option<R> {
        let label = literal.label.as_ref();
        let span = literal.span;
        let is_suspend = literal.is_suspend;
        let signature = match expected {
            Some(ExpressionExpectation::Type(ty)) => self.callable_expectation(ty),
            Some(ExpressionExpectation::Callable(signature)) => Some(signature),
            None => None,
        };
        match &literal.header {
            RecoveredCallableHeader::Lambda { parameters } => {
                let parameters = self.lambda_source_parameters(
                    is_suspend,
                    parameters.as_deref(),
                    span,
                    signature.as_ref(),
                )?;
                let receiver = signature.as_ref().and_then(|signature| signature.receiver);
                self.with_callable_literal_scope(
                    scope::LiteralKind::Lambda,
                    label,
                    is_suspend,
                    receiver.is_some(),
                    span,
                    |state, _, _| {
                        state.bind_literal_receiver(receiver, label, span);
                        state.lower_lambda_parameters(
                            &parameters,
                            signature
                                .as_ref()
                                .map(|signature| signature.parameter_types.as_slice()),
                            span,
                        )?;
                        let known_return = signature
                            .as_ref()
                            .and_then(|signature| signature.return_type);
                        state.current_return_ty = known_return.unwrap_or(state.unit);
                        state.return_inference =
                            known_return.is_none().then(ReturnInference::default);
                        Some(query(state))
                    },
                )
            }
            RecoveredCallableHeader::AnonymousFunction {
                receiver,
                parameters,
                return_ty,
            } => {
                let signature = self.anonymous_signature(
                    receiver.as_deref(),
                    is_suspend,
                    parameters,
                    return_ty.as_deref(),
                    span,
                    signature,
                )?;
                self.with_callable_literal_scope(
                    scope::LiteralKind::AnonymousFunction,
                    label,
                    is_suspend,
                    signature.receiver.is_some(),
                    span,
                    |state, _, _| {
                        state.current_return_ty = signature.result.unwrap_or(state.unit);
                        state.return_inference =
                            signature.result.is_none().then(ReturnInference::default);
                        state.bind_literal_receiver(signature.receiver, label, span);
                        state.bind_anonymous_parameters(parameters, &signature.parameters)?;
                        Some(query(state))
                    },
                )
            }
        }
    }
}
