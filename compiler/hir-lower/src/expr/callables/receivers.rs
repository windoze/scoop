use super::*;
use crate::expr::members::PropertyExtensionInvokeOutcome;

impl Lowerer {
    pub(in crate::expr) fn probe_receiver_function_call(
        &self,
        receiver: hir::Expr,
        name: &ast::Ident,
        call: CallSite<'_>,
    ) -> PropertyExtensionInvokeOutcome {
        let mut state = self.clone();
        let mut sink = Vec::new();
        // The explicit receiver precedes the property/variable read in source order.
        let receiver = state.materialize_temporary(
            "$function.receiver".into(),
            receiver,
            call.span,
            &mut sink,
        );
        let Some(callee) = state.lower_var(name, &mut sink, None) else {
            return PropertyExtensionInvokeOutcome::NoApplicable(None);
        };
        let Type::Function(function) = state.types[callee.ty] else {
            return PropertyExtensionInvokeOutcome::NoApplicable(None);
        };
        if state.function_types[function].receiver.is_none() {
            return PropertyExtensionInvokeOutcome::NoApplicable(None);
        }
        if !call.type_args.is_empty() {
            state.error(
                name.span,
                "function values do not accept explicit type arguments".into(),
            );
            return PropertyExtensionInvokeOutcome::NoApplicable(Some(Box::new(state)));
        }
        match state.lower_callable_with_receiver(
            callee,
            Some(receiver),
            call.args,
            call.span,
            &mut sink,
        ) {
            Some(expression) => PropertyExtensionInvokeOutcome::Resolved(SuccessfulExprLayer {
                state: Box::new(state),
                expression,
                sink,
            }),
            None => PropertyExtensionInvokeOutcome::NoApplicable(Some(Box::new(state))),
        }
    }
}
