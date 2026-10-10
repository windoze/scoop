//! Managed function-value calls preserve callee and source argument evaluation.

use super::*;

impl Lowerer {
    pub(in crate::expr) fn lower_callable_call(
        &mut self,
        callee: hir::Expr,
        args: &[ast::CallArgument],
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let Type::Function(function_type) = self.types[callee.ty] else {
            let found = self.type_name(callee.ty);
            self.error(
                callee.span,
                format!("value of type {found} is not callable"),
            );
            return None;
        };
        let signature = self.function_types[function_type].clone();
        if let Some(argument) = args
            .iter()
            .find(|argument| matches!(argument.name, ast::CallArgumentName::Named(_)))
        {
            self.error(
                argument.span,
                "function values do not accept named arguments".to_string(),
            );
            return None;
        }
        if let Some(argument) = args
            .iter()
            .find(|argument| matches!(argument.spread, ast::SpreadSyntax::Spread(_)))
        {
            self.error(
                argument.span,
                "function values do not accept spread arguments".to_string(),
            );
            return None;
        }
        if signature.parameter_types.len() != args.len() {
            self.error(
                span,
                format!(
                    "function value takes exactly {} argument(s), but {} were supplied",
                    signature.parameter_types.len(),
                    args.len()
                ),
            );
            return None;
        }
        if signature.is_suspend {
            let context = *self
                .suspension_contexts
                .last()
                .expect("the suspension context stack is initialized");
            if let SuspensionContext::Forbidden(reason) = context {
                let location = match reason {
                    ForbiddenSuspendContext::Release => "a `release` block".to_string(),
                    ForbiddenSuspendContext::TopLevel => "a non-suspend declaration".to_string(),
                    ForbiddenSuspendContext::Function => {
                        format!("non-suspend function `{}`", self.current_fn_name)
                    }
                    ForbiddenSuspendContext::DefaultExpression => {
                        format!("non-suspend default expression {}", self.current_fn_name)
                    }
                    ForbiddenSuspendContext::ConstructorDelegation => {
                        "constructor delegation".to_string()
                    }
                    ForbiddenSuspendContext::ConstructorInitialization => {
                        "constructor initialization".to_string()
                    }
                };
                self.error(
                    span,
                    format!("suspend function value cannot be called from {location}"),
                );
                return None;
            }
        }
        let mut prepared = Vec::with_capacity(args.len());
        for (arg, &parameter_ty) in args.iter().zip(&signature.parameter_types) {
            let mut setup = Vec::new();
            let value = self.lower_expr(&arg.expression, &mut setup, Some(parameter_ty))?;
            if !self.is_subtype(value.ty, parameter_ty) {
                let expected = self.type_name(parameter_ty);
                let found = self.type_name(value.ty);
                self.error(
                    arg.span,
                    format!("function argument must be of type {expected}, found {found}"),
                );
                return None;
            }
            prepared.push((self.adapt_to(value, parameter_ty), setup));
        }
        let last_setup = prepared.iter().rposition(|(_, setup)| !setup.is_empty());
        let callee = if last_setup.is_some() {
            self.materialize_temporary("$callable.receiver".into(), callee, span, sink)
        } else {
            callee
        };
        let mut lowered = Vec::with_capacity(args.len());
        for (index, (value, mut setup)) in prepared.into_iter().enumerate() {
            sink.append(&mut setup);
            let value = if last_setup.is_some_and(|last| index < last) {
                self.materialize_temporary(format!("$callable.argument.{index}"), value, span, sink)
            } else {
                value
            };
            lowered.push(value);
        }
        Some(hir::Expr {
            kind: ExprKind::CallableCall {
                callee: Box::new(callee),
                function_type,
                args: lowered,
            },
            ty: signature.return_type,
            span,
            origin: self.expression_origin(span),
        })
    }
}
