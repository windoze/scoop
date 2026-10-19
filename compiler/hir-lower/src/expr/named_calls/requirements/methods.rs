use super::*;
use crate::derived::DerivedEqualityCandidate;
use crate::expr::named_calls::imported_dependency::{ImportedMemberReceiver, ImportedProbeCall};

impl Lowerer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn requirement_method_probes(
        &mut self,
        subject: TypeId,
        receiver: hir::Expr,
        name: &str,
        types: &[hir::RequiredTypeArgument],
        arguments: &[ast::CallArgument],
        span: Span,
    ) -> Result<Vec<NamedApplicable>, String> {
        self.resolve_imported_member_receiver(subject, span)
            .map_err(|()| self.requirement_failure("invalid member receiver"))?;
        let explicit = types
            .iter()
            .map(|ty| match ty {
                hir::RequiredTypeArgument::Explicit(ty) => {
                    ResolvedCallTypeArgument::Explicit { ty: *ty, span }
                }
                hir::RequiredTypeArgument::Infer => ResolvedCallTypeArgument::Infer { span },
            })
            .collect::<Vec<_>>();
        let ident = ast::Ident {
            text: name.to_owned(),
            span,
        };
        let call = CallSite {
            type_args: &[],
            args: arguments,
            span,
        };
        let mut candidates = self.public_requirement_methods(subject, name);
        let imported = self
            .imported_requirement_members(subject, hir::ImportedMemberLookup::Name(name))
            .map_err(|error| error.to_string())?;
        let mut probes = Vec::new();
        let mut failure = None;
        if name == "equals" {
            match self.derived_equality_candidate(subject, span) {
                Ok(Some(DerivedEqualityCandidate::Imported(target))) if types.is_empty() => {
                    match self.probe_imported_derived_equality(target, receiver.clone(), call, None)
                    {
                        Ok(probe) => probes.push(NamedFunctionLikeProbe::ImportedDerivedEquality(
                            Box::new(probe),
                        )),
                        Err(state) => {
                            failure = Some(state.requirement_failure("invalid derived equality"))
                        }
                    }
                }
                Ok(Some(DerivedEqualityCandidate::Nominal { overload, .. })) => {
                    if !candidates
                        .iter()
                        .any(|candidate| candidate.function == overload.function)
                    {
                        candidates.push(overload);
                    }
                }
                Ok(Some(DerivedEqualityCandidate::TypeOwned { function, .. })) => {
                    candidates.push(crate::CallableCandidate::type_owned(function, subject))
                }
                Err(error) => failure = Some(error),
                _ => {}
            }
        }
        for candidate in candidates {
            match self.probe_named_callable(
                name,
                candidate,
                NamedCallReceiver::Member(receiver.clone()),
                OverloadCall {
                    explicit_type_args: &explicit,
                    arg_exprs: arguments,
                    span,
                    expected_result: None,
                    argument_protocol: CallArgumentProtocol::Ordinary,
                },
            ) {
                Ok(probe) => probes.push(NamedFunctionLikeProbe::Callable(Box::new(probe))),
                Err(state) => {
                    failure.get_or_insert_with(|| {
                        state.requirement_failure("required member is inapplicable")
                    });
                }
            }
        }
        for candidate in imported {
            match self.probe_imported_member_callable(
                candidate,
                ImportedMemberReceiver::Value(receiver.clone()),
                &ident,
                ImportedProbeCall::from(call).with_resolved_type_arguments(&explicit),
                None,
                false,
            ) {
                Ok(probe) => {
                    probes.push(NamedFunctionLikeProbe::ImportedDependency(Box::new(probe)))
                }
                Err(state) => {
                    failure.get_or_insert_with(|| {
                        state.requirement_failure("required dependency member is inapplicable")
                    });
                }
            }
        }
        if probes.is_empty()
            && let Some(error) = failure
        {
            return Err(error);
        }
        Ok(probes
            .into_iter()
            .map(|probe| NamedApplicable {
                commit: match probe {
                    NamedFunctionLikeProbe::ImportedDependency(_) => {
                        NamedFunctionCommit::ImportedDependency
                    }
                    _ => NamedFunctionCommit::Member,
                },
                probe,
            })
            .collect())
    }
}
