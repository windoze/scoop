use super::*;

impl Lowerer {
    pub(crate) fn lower_named_call_on_receiver(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
        required: RequiredCallableModifiers,
    ) -> Option<hir::Expr> {
        if name.text == "invoke" && matches!(self.types[receiver.ty], Type::Function(_)) {
            if required.operator.is_some()
                || required.property_delegate_operator.is_some()
                || required.infix
            {
                let found = self.type_name(receiver.ty);
                self.error(
                    name.span,
                    format!(
                        "type `{found}` has no matching callable role `{}`",
                        name.text
                    ),
                );
                return None;
            }
            if !call.type_args.is_empty() {
                self.error(
                    name.span,
                    "function values do not accept explicit type arguments".to_string(),
                );
                return None;
            }
            return self.lower_callable_call(receiver, call.args, call.span, sink);
        }
        let mut candidates = match required.operator {
            Some(operator) => self.methods_by_operator(receiver.ty, operator),
            None => self.methods_by_name(receiver.ty, &name.text),
        };
        candidates.retain(|candidate| {
            let modifiers = self.signatures[&candidate.function].modifiers;
            Self::matches_required_modifiers(modifiers, required)
        });
        let mut first_failure = None;
        if !candidates.is_empty() && matches!(self.types[receiver.ty], Type::Interface(..)) {
            let before = candidates.len();
            candidates.retain(|candidate| {
                let sig = &self.signatures[&candidate.function];
                sig.type_params.len() == sig.owner_type_param_count
            });
            if candidates.is_empty() && before != 0 {
                let found = self.type_name(receiver.ty);
                let mut failure = self.clone();
                failure.error(
                    name.span,
                    format!(
                        "generic member function `{}` cannot be called through interface type `{found}`",
                        name.text
                    ),
                );
                first_failure = Some(Box::new(failure));
            }
        }
        match self.probe_member_call_partition(
            candidates,
            name,
            receiver.clone(),
            call,
            expected,
            required,
        ) {
            PropertyExtensionInvokeOutcome::Resolved(layer) => {
                return Some(self.commit_expr_layer(layer, sink));
            }
            PropertyExtensionInvokeOutcome::Blocked => return None,
            PropertyExtensionInvokeOutcome::Failed(failure) => {
                self.commit_layer_diagnostics(*failure);
                return None;
            }
            PropertyExtensionInvokeOutcome::NoApplicable(failure) => {
                if let Some(failure) = failure {
                    first_failure.get_or_insert(failure);
                }
            }
        }

        let extension_layers = match required.operator {
            Some(operator) => self.named_executable_extension_operator_layers(operator),
            None => self.named_executable_extension_call_layers(&name.text),
        };
        for layer in extension_layers {
            let suppressed = layer.suppressed_callables.iter().copied().any(|function| {
                Self::matches_required_modifiers(self.signatures[&function].modifiers, required)
            });
            let extensions = layer
                .candidates
                .into_iter()
                .filter(|target| self.extension_call_target_matches_required(target, required))
                .collect::<Vec<_>>();
            if extensions.is_empty() {
                if suppressed {
                    return None;
                }
                continue;
            }
            match self.probe_extension_call_partition(
                &extensions,
                name,
                receiver.clone(),
                call,
                expected,
                required.operator == Some(hir::OperatorKind::Set),
                "extension candidate",
            ) {
                PropertyExtensionInvokeOutcome::Resolved(layer) => {
                    return Some(self.commit_expr_layer(layer, sink));
                }
                PropertyExtensionInvokeOutcome::Blocked => return None,
                PropertyExtensionInvokeOutcome::Failed(failure) => {
                    self.commit_layer_diagnostics(*failure);
                    return None;
                }
                PropertyExtensionInvokeOutcome::NoApplicable(failure) => {
                    if let Some(failure) = failure {
                        if suppressed {
                            self.commit_layer_diagnostics(*failure);
                            return None;
                        }
                        first_failure.get_or_insert(failure);
                    } else if suppressed {
                        return None;
                    }
                }
            }
        }

        if let Some(failure) = first_failure {
            self.commit_layer_diagnostics(*failure);
        } else {
            let found = self.type_name(receiver.ty);
            self.error(
                name.span,
                format!("type `{found}` has no method `{}`", name.text),
            );
        }
        None
    }

    pub(in crate::expr) fn lower_infix_call(
        &mut self,
        lhs: &ast::Expr,
        target: &ast::InfixTarget,
        rhs: &ast::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let args = [ast::CallArgument::positional(rhs.clone())];
        let call = CallSite {
            type_args: &[],
            args: &args,
            span,
        };
        if let ast::InfixTarget::Named(name) = target
            && let Some(layer) =
                self.probe_integer_literal_receiver(lhs, expected, |state, receiver, layer_sink| {
                    state.lower_explicit_named_call(
                        receiver,
                        name,
                        call,
                        layer_sink,
                        expected,
                        RequiredCallableModifiers {
                            operator: None,
                            infix: true,
                            ..Default::default()
                        },
                    )
                })
        {
            return Some(self.commit_expr_layer(layer, sink));
        }
        let receiver = self.lower_expr(lhs, sink, None)?;
        match target {
            ast::InfixTarget::Named(name) => self.lower_explicit_named_call(
                receiver,
                name,
                call,
                sink,
                expected,
                RequiredCallableModifiers {
                    operator: None,
                    infix: true,
                    ..Default::default()
                },
            ),
            ast::InfixTarget::Invoke => {
                self.lower_value_invoke(receiver, call, sink, expected, true)
            }
        }
    }
}
