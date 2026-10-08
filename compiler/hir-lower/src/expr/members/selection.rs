use super::*;
use crate::call_resolution::named::NamedFunctionLikeProbe;
use crate::derived::DerivedEqualityCandidate;
use crate::expr::named_calls::imported_dependency::ImportedMemberReceiver;
use crate::overload::{CallArgumentProtocol, NamedCallReceiver, OverloadCall};

impl Lowerer {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::expr) fn probe_member_call_partition(
        &self,
        candidates: Vec<crate::CallableCandidate>,
        name: &ast::Ident,
        receiver: hir::Expr,
        call: CallSite<'_>,
        expected: Option<TypeId>,
        required: RequiredCallableModifiers,
    ) -> PropertyExtensionInvokeOutcome {
        self.probe_member_call_partition_with_kind(
            candidates,
            name,
            receiver,
            call,
            expected,
            required,
            MemberCallKind::Ordinary,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn probe_member_call_partition_with_kind(
        &self,
        mut candidates: Vec<crate::CallableCandidate>,
        name: &ast::Ident,
        receiver: hir::Expr,
        call: CallSite<'_>,
        expected: Option<TypeId>,
        required: RequiredCallableModifiers,
        kind: MemberCallKind,
    ) -> PropertyExtensionInvokeOutcome {
        let mut context = self.clone();
        let imported =
            match context.imported_member_call_candidates(receiver.ty, name, required, kind) {
                Ok(candidates) => candidates,
                Err(failure) => return PropertyExtensionInvokeOutcome::Failed(failure),
            };
        let imported = imported
            .into_iter()
            .map(|candidate| (candidate, receiver.clone()))
            .collect::<Vec<_>>();
        let mut imported_equality = None;
        if kind == MemberCallKind::Ordinary
            && name.text == "equals"
            && !required.infix
            && required.property_delegate_operator.is_none()
            && required
                .operator
                .is_none_or(|operator| operator == hir::OperatorKind::Equals)
        {
            match context.derived_equality_candidate(receiver.ty, call.span) {
                Ok(Some(DerivedEqualityCandidate::Imported(target))) => {
                    imported_equality = Some(target)
                }
                Ok(Some(DerivedEqualityCandidate::Nominal {
                    overload: candidate,
                    ..
                })) => {
                    if !candidates
                        .iter()
                        .any(|existing| existing.function == candidate.function)
                    {
                        candidates.push(candidate);
                    }
                }
                Ok(Some(DerivedEqualityCandidate::TypeOwned { function, .. })) => {
                    candidates.push(crate::CallableCandidate::type_owned(function, receiver.ty));
                }
                Err(reason) if candidates.is_empty() && imported.is_empty() => {
                    context.error(call.span, reason);
                    return PropertyExtensionInvokeOutcome::NoApplicable(Some(Box::new(context)));
                }
                Ok(None) | Err(_) => {}
            }
        }
        let operator_set = required.operator == Some(hir::OperatorKind::Set);
        if imported.is_empty() && imported_equality.is_none() && kind == MemberCallKind::Ordinary {
            return context.probe_local_member_call_partition(
                candidates,
                &name.text,
                receiver,
                call,
                expected,
                operator_set,
            );
        }
        let mut probes = Vec::new();
        let mut first_failure = None;
        if let Some(target) = imported_equality {
            match context.probe_imported_derived_equality(target, receiver.clone(), call, expected)
            {
                Ok(probe) => probes.push(NamedFunctionLikeProbe::ImportedDerivedEquality(
                    Box::new(probe),
                )),
                Err(failure) => {
                    first_failure = Some(failure);
                }
            }
        }
        let mut suppressed = false;
        for candidate in candidates {
            if context
                .declaration_surface
                .rejects_function(candidate.function)
            {
                suppressed = true;
                continue;
            }
            let mut state = context.clone();
            let Some(explicit_type_args) = state.resolve_call_type_args(call.type_args) else {
                first_failure.get_or_insert(Box::new(state));
                continue;
            };
            match state.probe_named_callable(
                &name.text,
                candidate,
                NamedCallReceiver::Member(receiver.clone()),
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
                Ok(probe) => probes.push(NamedFunctionLikeProbe::Callable(Box::new(probe))),
                Err(failure) => {
                    first_failure.get_or_insert(failure);
                }
            }
        }
        for (candidate, receiver) in imported {
            match context.probe_imported_member_callable(
                candidate,
                ImportedMemberReceiver::Value(receiver),
                name,
                call.into(),
                expected,
                operator_set,
            ) {
                Ok(probe) => {
                    probes.push(NamedFunctionLikeProbe::ImportedDependency(Box::new(probe)))
                }
                Err(failure) => {
                    first_failure.get_or_insert(failure);
                }
            }
        }
        if probes.is_empty() {
            return match (suppressed, first_failure) {
                (true, Some(failure)) => PropertyExtensionInvokeOutcome::Failed(failure),
                (true, None) => PropertyExtensionInvokeOutcome::Blocked,
                (false, failure) => PropertyExtensionInvokeOutcome::NoApplicable(failure),
            };
        }
        let mut state = context;
        let Some(winner) =
            state.select_named_function_like(&name.text, "member", &probes, call.args, call.span)
        else {
            return PropertyExtensionInvokeOutcome::Failed(Box::new(state));
        };
        let mut sink = Vec::new();
        let expression = match probes.swap_remove(winner) {
            NamedFunctionLikeProbe::Callable(probe) => state
                .commit_named_callable(*probe, &mut sink)
                .and_then(|resolved| match kind {
                    MemberCallKind::Ordinary => {
                        state.finish_resolved_method_call(resolved, call.span, &sink)
                    }
                    MemberCallKind::DirectSuper => {
                        state.finish_resolved_super_method_call(resolved, &name.text, call.span)
                    }
                }),
            NamedFunctionLikeProbe::ImportedDependency(probe) => {
                state.commit_imported_dependency_callable_with_kind(*probe, &mut sink, kind)
            }
            NamedFunctionLikeProbe::ImportedDerivedEquality(probe) => {
                Some((*probe).commit(&mut state, &mut sink))
            }
            _ => unreachable!("member call candidates are callable declarations"),
        };
        match expression {
            Some(expression) => PropertyExtensionInvokeOutcome::Resolved(SuccessfulExprLayer {
                state: Box::new(state),
                expression,
                sink,
            }),
            None => PropertyExtensionInvokeOutcome::Failed(Box::new(state)),
        }
    }
}
