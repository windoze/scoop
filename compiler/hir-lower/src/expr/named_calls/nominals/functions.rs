use super::*;

impl Lowerer {
    pub(super) fn collect_named_function_probes(
        &self,
        targets: &[NamedCallBinding],
        call: &ast::CallExpr,
        overload: OverloadCall<'_>,
        applicable: &mut Vec<NamedApplicable>,
        failures: &mut Vec<Box<Lowerer>>,
    ) {
        let (extensions, ordinary): (Vec<_>, Vec<_>) =
            targets
                .iter()
                .partition(|binding| match (&binding.target, &binding.origin) {
                    (NamedCallTarget::Function(function), _) => {
                        self.extension_receivers.contains_key(function)
                    }
                    (
                        NamedCallTarget::ImportedDependency(_),
                        NamedCallOrigin::Dependency(binding),
                    ) => self
                        .dependencies
                        .as_ref()
                        .and_then(|dependencies| dependencies.callable_candidate(binding).ok())
                        .is_some_and(|candidate| {
                            candidate.interface().owner()
                                == hir::PublicDeclarationOwnerV1::Extension
                        }),
                    _ => false,
                });
        for target in ordinary {
            self.collect_named_function_probe(target, None, call, overload, applicable, failures);
        }
        for receiver in self.implicit_receivers() {
            let before = applicable.len();
            for target in &extensions {
                self.collect_named_function_probe(
                    target,
                    Some(receiver),
                    call,
                    overload,
                    applicable,
                    failures,
                );
            }
            if applicable.len() != before {
                break;
            }
        }
    }

    fn collect_named_function_probe(
        &self,
        binding: &NamedCallBinding,
        receiver: Option<crate::ReceiverBinding>,
        call: &ast::CallExpr,
        overload: OverloadCall<'_>,
        applicable: &mut Vec<NamedApplicable>,
        failures: &mut Vec<Box<Lowerer>>,
    ) {
        if let NamedCallTarget::Function(function) = binding.target {
            let mut state = self.clone();
            let (candidate, receiver, commit) = if let Some(receiver) = receiver {
                let Some(receiver) = state.lower_receiver_binding(receiver, call.callee.span)
                else {
                    return;
                };
                (
                    crate::CallableCandidate::function(function, Vec::new()),
                    NamedCallReceiver::Extension(receiver),
                    NamedFunctionCommit::TopLevel,
                )
            } else if let Some(crate::Owner::Object(object)) =
                self.function_owner.get(&function).copied()
            {
                let Some(receiver) = state.lower_singleton_value(object, call.callee.span) else {
                    return;
                };
                let candidate = crate::CallableCandidate {
                    function,
                    owner: crate::CallableCandidateOwner::Method(
                        state.method_owner_application(crate::Owner::Object(object), Vec::new()),
                    ),
                    source: crate::CallableCandidateSource::Direct,
                };
                (
                    candidate,
                    NamedCallReceiver::Member(receiver),
                    NamedFunctionCommit::Member,
                )
            } else {
                (
                    crate::CallableCandidate::function(function, Vec::new()),
                    NamedCallReceiver::None,
                    NamedFunctionCommit::TopLevel,
                )
            };
            match state.probe_named_callable(&call.callee.text, candidate, receiver, overload) {
                Ok(probe) => applicable.push(NamedApplicable {
                    probe: NamedFunctionLikeProbe::Callable(Box::new(probe)),
                    commit,
                }),
                Err(failure) => failures.push(failure),
            }
            return;
        }
        if !matches!(
            binding.target,
            NamedCallTarget::ImportedDependency(
                hir::ImportedTarget::Function(_)
                    | hir::ImportedTarget::GenericFunction(_)
                    | hir::ImportedTarget::EnumVariant(_)
            )
        ) {
            return;
        }
        let NamedCallOrigin::Dependency(binding) = &binding.origin else {
            unreachable!("an ordinary dependency target retains its dependency binding")
        };
        let probe = if let Some(receiver) = receiver {
            let mut state = self.clone();
            let Some(receiver) = state.lower_receiver_binding(receiver, call.callee.span) else {
                return;
            };
            state.probe_imported_dependency_extension_callable(
                binding,
                receiver,
                &call.callee,
                CallSite {
                    type_args: &call.type_args,
                    args: &call.args,
                    span: call.span,
                },
                overload.expected_result,
                false,
            )
        } else {
            self.probe_imported_dependency_callable(binding, call, overload.expected_result)
        };
        match probe {
            Ok(probe) => applicable.push(NamedApplicable {
                probe: NamedFunctionLikeProbe::ImportedDependency(Box::new(probe)),
                commit: NamedFunctionCommit::ImportedDependency,
            }),
            Err(failure) => failures.push(failure),
        }
    }
}
