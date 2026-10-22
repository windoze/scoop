use super::*;
use crate::call_resolution::named::NamedFunctionLikeProbe;
use crate::imports::lookup::calls::{ExtensionCallTarget, wire_operator};
use crate::overload::{CallArgumentProtocol, NamedCallReceiver, OverloadCall};

impl Lowerer {
    pub(in crate::expr) fn extension_call_target_matches_required(
        &self,
        target: &ExtensionCallTarget,
        required: RequiredCallableModifiers,
    ) -> bool {
        match target {
            ExtensionCallTarget::Current(function) => {
                self.function_matches_required_modifiers(*function, required)
            }
            ExtensionCallTarget::Dependency(binding) => {
                let Some(candidate) = self
                    .dependencies
                    .as_ref()
                    .and_then(|dependencies| dependencies.callable_candidate(binding).ok())
                else {
                    return true;
                };
                required.operator.is_none_or(|operator| {
                    candidate.interface().effects().operator_role()
                        == hir::CallableOperatorRoleV1::Language(wire_operator(operator))
                }) && required.property_delegate_operator.is_none_or(|operator| {
                    candidate.interface().effects().operator_role()
                        == hir::CallableOperatorRoleV1::PropertyDelegate(
                            imported_delegate_operator(operator),
                        )
                }) && (!required.infix
                    || candidate.interface().effects().infix() == hir::CallableInfixV1::Infix)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn probe_extension_call_partition(
        &self,
        candidates: &[ExtensionCallTarget],
        name: &ast::Ident,
        receiver: hir::Expr,
        call: CallSite<'_>,
        expected: Option<TypeId>,
        operator_set: bool,
        layer_name: &str,
    ) -> PropertyExtensionInvokeOutcome {
        if candidates.is_empty() {
            return PropertyExtensionInvokeOutcome::NoApplicable(None);
        }
        let mut context = self.clone();
        let views = context.smart_cast_receiver_views(&receiver);
        let mut probes = Vec::new();
        let mut first_failure = None;
        let mut wrong_arity_failure = None;
        let mut suppressed = false;
        for target in candidates {
            let failure_slot = if call.type_args.is_empty()
                || self.extension_call_target_has_type_arity(target, call.type_args.len())
            {
                &mut first_failure
            } else {
                &mut wrong_arity_failure
            };
            for receiver in &views {
                match target {
                    ExtensionCallTarget::Current(function) => {
                        if self.declaration_surface.rejects_function(*function) {
                            suppressed = true;
                            continue;
                        }
                        let mut state = context.clone();
                        let Some(explicit_type_args) = state.resolve_call_type_args(call.type_args)
                        else {
                            failure_slot.get_or_insert(Box::new(state));
                            continue;
                        };
                        let candidate = crate::CallableCandidate::function(*function, Vec::new());
                        match state.probe_named_callable(
                            &name.text,
                            candidate,
                            NamedCallReceiver::Extension(receiver.clone()),
                            OverloadCall {
                                explicit_type_args: &explicit_type_args,
                                arg_exprs: call.args,
                                span: call.span,
                                expected_result: expected,
                                argument_protocol: if operator_set {
                                    CallArgumentProtocol::OperatorSet
                                } else {
                                    CallArgumentProtocol::Ordinary
                                },
                            },
                        ) {
                            Ok(probe) => {
                                probes.push(NamedFunctionLikeProbe::Callable(Box::new(probe)));
                                break;
                            }
                            Err(failure) => {
                                failure_slot.get_or_insert(failure);
                            }
                        }
                    }
                    ExtensionCallTarget::Dependency(binding) => {
                        match context.probe_imported_dependency_extension_callable(
                            binding,
                            receiver.clone(),
                            name,
                            call,
                            expected,
                            operator_set,
                        ) {
                            Ok(probe) => {
                                probes.push(NamedFunctionLikeProbe::ImportedDependency(Box::new(
                                    probe,
                                )));
                                break;
                            }
                            Err(failure) => {
                                failure_slot.get_or_insert(failure);
                            }
                        }
                    }
                }
            }
        }
        if probes.is_empty() {
            let first_failure = first_failure.or(wrong_arity_failure);
            return match (suppressed, first_failure) {
                (true, Some(failure)) => PropertyExtensionInvokeOutcome::Failed(failure),
                (true, None) => PropertyExtensionInvokeOutcome::Blocked,
                (false, failure) => PropertyExtensionInvokeOutcome::NoApplicable(failure),
            };
        }

        let mut state = context;
        let Some(winner) =
            state.select_named_function_like(&name.text, layer_name, &probes, call.args, call.span)
        else {
            return PropertyExtensionInvokeOutcome::Failed(Box::new(state));
        };
        match probes.swap_remove(winner) {
            NamedFunctionLikeProbe::Callable(probe) => {
                let mut layer_sink = Vec::new();
                let Some(resolved) = state.commit_named_callable(*probe, &mut layer_sink) else {
                    return PropertyExtensionInvokeOutcome::Failed(Box::new(state));
                };
                state.observe_callable_reference(
                    name.span,
                    hir::SemanticCallable::Current(resolved.function()),
                );
                let callee = state.materialize_resolved_callee(&resolved);
                state.check_call_effects(callee, call.span);
                let expression = hir::Expr {
                    kind: hir::ExprKind::Call {
                        binding: None,
                        callee: callee.into(),
                        receiver: resolved.source_receiver,
                        args: resolved.args,
                    },
                    ty: resolved.return_ty,
                    span: call.span,
                    origin: state.expression_origin(call.span),
                };
                PropertyExtensionInvokeOutcome::Resolved(SuccessfulExprLayer {
                    state: Box::new(state),
                    expression,
                    sink: layer_sink,
                })
            }
            NamedFunctionLikeProbe::ImportedDependency(probe) => {
                let target = probe.semantic_target();
                let mut layer_sink = Vec::new();
                let Some(expression) =
                    state.commit_imported_dependency_callable(*probe, &mut layer_sink)
                else {
                    return PropertyExtensionInvokeOutcome::Failed(Box::new(state));
                };
                if let Some(target) = target {
                    state.observe_callable_reference(name.span, target);
                }
                PropertyExtensionInvokeOutcome::Resolved(SuccessfulExprLayer {
                    state: Box::new(state),
                    expression,
                    sink: layer_sink,
                })
            }
            NamedFunctionLikeProbe::ImportedDerivedEquality(_)
            | NamedFunctionLikeProbe::ImportedDependencyProperty(_)
            | NamedFunctionLikeProbe::Nominal(_)
            | NamedFunctionLikeProbe::IntrinsicStruct(_) => {
                unreachable!("extension function partitions contain only callable candidates")
            }
        }
    }

    fn extension_call_target_has_type_arity(
        &self,
        target: &ExtensionCallTarget,
        arity: usize,
    ) -> bool {
        match target {
            ExtensionCallTarget::Current(function) => {
                let signature = &self.signatures[function];
                signature.type_params.len() - signature.owner_type_param_count == arity
            }
            ExtensionCallTarget::Dependency(binding) => self
                .dependencies
                .as_ref()
                .and_then(|dependencies| dependencies.callable_candidate(binding).ok())
                .is_none_or(|candidate| {
                    candidate.interface().type_parameters().binders().len() == arity
                }),
        }
    }
}

pub(super) const fn imported_delegate_operator(
    operator: hir::PropertyDelegateOperatorKind,
) -> hir::PropertyDelegateOperatorV1 {
    match operator {
        hir::PropertyDelegateOperatorKind::ProvideDelegate => {
            hir::PropertyDelegateOperatorV1::ProvideDelegate
        }
        hir::PropertyDelegateOperatorKind::GetValue => hir::PropertyDelegateOperatorV1::GetValue,
        hir::PropertyDelegateOperatorKind::SetValue => hir::PropertyDelegateOperatorV1::SetValue,
    }
}
