use super::*;
use crate::constructor_resolution::NominalConstructorCall;

mod plans;

struct NominalPlan {
    view: NominalConstructorView,
    expected: Option<TypeId>,
    fixed_alias: bool,
}

enum PreparedNominalPlans {
    Local(Vec<NominalPlan>, Option<(hir::StructId, Option<TypeId>)>),
    Imported(scoop_identity::PersistentTypeId),
}

impl Lowerer {
    pub(super) fn lower_named_function_partition(
        &mut self,
        targets: &[NamedCallBinding],
        kind: ImportLookupLayer,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Result<Option<hir::Expr>, ()> {
        let mut plans = Vec::new();
        let diagnostics_before = self.diagnostics.len();
        let mut failures = Vec::new();
        let mut intrinsics = Vec::new();
        let mut imported = Vec::new();
        for binding in targets {
            let mut preparation = self.clone();
            match preparation.named_nominal_plans(binding, call, expected) {
                Ok(PreparedNominalPlans::Local(mut prepared, intrinsic)) => {
                    *self = preparation;
                    plans.append(&mut prepared);
                    if let Some(intrinsic) = intrinsic {
                        intrinsics.push(intrinsic);
                    }
                }
                Ok(PreparedNominalPlans::Imported(owner)) => {
                    imported.push((preparation, owner));
                }
                Err(()) => {
                    failures.push(Box::new(preparation));
                }
            }
        }
        let explicit = self.resolve_call_type_args(&call.type_args).ok_or(())?;
        let overload = OverloadCall {
            explicit_type_args: &explicit,
            arg_exprs: &call.args,
            span: call.span,
            expected_result: expected,
            argument_protocol: CallArgumentProtocol::Ordinary,
        };
        let functions = targets
            .iter()
            .filter_map(|binding| match binding.target {
                NamedCallTarget::Function(id) => Some(id),
                _ => None,
            })
            .collect::<Vec<_>>();
        if plans.is_empty()
            && intrinsics.is_empty()
            && !functions.is_empty()
            && functions.len() == targets.len()
            && functions.iter().all(|id| {
                !self.extension_receivers.contains_key(id) && !self.function_owner.contains_key(id)
            })
        {
            return match self.resolve_overload_outcome(
                &call.callee.text,
                &functions,
                &[],
                overload,
                sink,
            ) {
                crate::overload::OverloadResolutionOutcome::NoApplicable => Ok(None),
                crate::overload::OverloadResolutionOutcome::Blocked => Ok(None),
                crate::overload::OverloadResolutionOutcome::Failed => Err(()),
                crate::overload::OverloadResolutionOutcome::Resolved(resolved) => self
                    .finish_resolved_top_level_function_call(call, *resolved, sink)
                    .map(Some)
                    .ok_or(()),
            };
        }
        let mut applicable = Vec::new();
        for binding in targets {
            let NamedCallTarget::Function(function) = binding.target else {
                continue;
            };
            let mut state = self.clone();
            let (candidate, receiver, commit) = if self.extension_receivers.contains_key(&function)
            {
                if self.current_this_ty().is_none() {
                    continue;
                }
                let Some(receiver) = state.lower_current_this(call.callee.span) else {
                    continue;
                };
                (
                    crate::CallableCandidate::function(
                        function,
                        Vec::new(),
                        state.function_lookup_witness(function),
                    ),
                    NamedCallReceiver::Extension(receiver),
                    NamedFunctionCommit::TopLevel,
                )
            } else if let Some(crate::Owner::Object(object)) =
                self.function_owner.get(&function).copied()
            {
                let Some(receiver) = state.lower_singleton_value(object, call.callee.span) else {
                    continue;
                };
                let candidate = crate::CallableCandidate {
                    function,
                    owner: crate::CallableCandidateOwner::Method(
                        state.method_owner_application(crate::Owner::Object(object), Vec::new()),
                    ),
                    source: crate::CallableCandidateSource::Direct,
                    access: crate::CallableCandidateAccess::Lookup(
                        state.function_lookup_witness(function),
                    ),
                };
                (
                    candidate,
                    NamedCallReceiver::Member(receiver),
                    NamedFunctionCommit::Member,
                )
            } else {
                (
                    crate::CallableCandidate::function(
                        function,
                        Vec::new(),
                        state.function_lookup_witness(function),
                    ),
                    NamedCallReceiver::None,
                    NamedFunctionCommit::TopLevel,
                )
            };
            match state.probe_named_callable(&call.callee.text, candidate, receiver, overload) {
                Ok(probe) => applicable.push(NamedApplicable {
                    probe: NamedFunctionLikeProbe::Callable(Box::new(probe)),
                    commit,
                }),
                Err(failure) => {
                    failures.push(failure);
                }
            }
        }
        for binding in targets {
            if !matches!(
                binding.target,
                NamedCallTarget::ImportedDependency(
                    hir::ImportedTarget::Function(_)
                        | hir::ImportedTarget::GenericFunction(_)
                        | hir::ImportedTarget::EnumVariant(_)
                )
            ) {
                continue;
            }
            let crate::imports::lookup::calls::NamedCallOrigin::Dependency(binding) =
                &binding.origin
            else {
                unreachable!("an ordinary dependency target retains its dependency binding")
            };
            match self.probe_imported_dependency_callable(binding, call, expected) {
                Ok(probe) => applicable.push(NamedApplicable {
                    probe: NamedFunctionLikeProbe::ImportedDependency(Box::new(probe)),
                    commit: NamedFunctionCommit::ImportedDependency,
                }),
                Err(failure) => failures.push(failure),
            }
        }
        for (mut state, owner) in imported {
            let candidates = state
                .dependencies
                .as_ref()
                .expect("dependency name lookup retains its declaration catalog")
                .constructor_candidates(owner);
            let candidates = match candidates {
                Ok(candidates) => candidates,
                Err(error) => {
                    state.error(
                        call.span,
                        format!("invalid dependency constructor declaration: {error}"),
                    );
                    failures.push(Box::new(state));
                    continue;
                }
            };
            if candidates.is_empty() {
                state.error(
                    call.span,
                    format!(
                        "type `{}` does not name a constructible type",
                        call.callee.text
                    ),
                );
                failures.push(Box::new(state));
                continue;
            }
            for candidate in candidates {
                match state.probe_imported_constructor(candidate, call, expected) {
                    Ok(probe) => applicable.push(NamedApplicable {
                        probe: NamedFunctionLikeProbe::ImportedDependency(Box::new(probe)),
                        commit: NamedFunctionCommit::ImportedDependency,
                    }),
                    Err(failure) => failures.push(failure),
                }
            }
        }
        for plan in plans {
            let arguments =
                self.named_nominal_expected_arguments(plan.view.result_type, plan.expected);
            match self.probe_named_nominal(
                plan.view,
                NominalConstructorCall {
                    explicit_type_args: &explicit,
                    expected_type_args: arguments.as_deref(),
                    arguments: &call.args,
                    span: call.span,
                },
            ) {
                Ok(mut probe) => {
                    if plan.fixed_alias {
                        probe.fix_forwarding_parameters(
                            self,
                            arguments
                                .as_deref()
                                .expect("a fixed nominal alias supplies its complete application"),
                        );
                    }
                    applicable.push(NamedApplicable {
                        probe: NamedFunctionLikeProbe::Nominal(Box::new(probe)),
                        commit: NamedFunctionCommit::Nominal,
                    });
                }
                Err(failure) => {
                    failures.push(failure);
                }
            }
        }
        for (structure, fixed) in intrinsics {
            match self.probe_expr_layer(|state, sink| {
                state.lower_ffi_struct_init(
                    structure,
                    CallSite {
                        type_args: &call.type_args,
                        args: &call.args,
                        span: call.span,
                    },
                    sink,
                    fixed,
                )
            }) {
                Ok(layer) => {
                    let ulong = self.integer_type(hir::IntegerKind::UNSIGNED_64);
                    let probe = crate::call_resolution::named::NamedIntrinsicStructProbe {
                        structure,
                        owners: self.structs[structure].type_params.clone(),
                        parameter_types: vec![ulong],
                        integer_arguments: vec![Some(hir::IntegerKind::UNSIGNED_64)],
                    };
                    applicable.push(NamedApplicable {
                        probe: NamedFunctionLikeProbe::IntrinsicStruct(probe),
                        commit: NamedFunctionCommit::Intrinsic(layer),
                    });
                }
                Err(failure) => {
                    failures.push(failure);
                }
            }
        }
        if applicable.is_empty() {
            let functions = targets
                .iter()
                .filter_map(|binding| match binding.target {
                    NamedCallTarget::Function(id)
                        if !self.extension_receivers.contains_key(&id)
                            && !self.function_owner.contains_key(&id) =>
                    {
                        Some(id)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            if functions.len() > 1
                && targets
                    .iter()
                    .all(|binding| matches!(binding.target, NamedCallTarget::Function(_)))
            {
                return Ok(self.lower_top_level_function_layer(
                    &call.callee.text,
                    &functions,
                    call,
                    sink,
                    expected,
                ));
            }
            if failures.len() == 1 {
                self.commit_layer_diagnostics(*failures.pop().expect("one failed candidate"));
            } else if !failures.is_empty() {
                let mut traces = failures
                    .iter()
                    .map(|failure| {
                        failure.diagnostics[diagnostics_before..]
                            .iter()
                            .map(|diagnostic| diagnostic.message.as_str())
                            .collect::<Vec<_>>()
                            .join("; ")
                    })
                    .collect::<Vec<_>>();
                traces.sort();
                self.error(
                    call.span,
                    format!(
                        "no applicable candidate for `{}` in {} layer:\n{}",
                        call.callee.text,
                        kind.call_name(),
                        traces
                            .iter()
                            .map(|trace| format!("  - {trace}"))
                            .collect::<Vec<_>>()
                            .join("\n")
                    ),
                );
            }
            return Ok(None);
        }
        let (probes, mut commits): (Vec<_>, Vec<_>) = applicable
            .into_iter()
            .map(|applicable| (applicable.probe, applicable.commit))
            .unzip();
        let diagnostics_before = self.diagnostics.len();
        let Some(winner) = self.select_named_function_like(
            &call.callee.text,
            kind.call_name(),
            &probes,
            &call.args,
            call.span,
        ) else {
            let variants = targets
                .iter()
                .filter_map(|binding| match binding.target {
                    NamedCallTarget::Value(ValueTarget::Variant(target)) => Some(target),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if kind == ImportLookupLayer::CorePrelude && variants.len() == targets.len() {
                self.diagnostics.truncate(diagnostics_before);
                self.ambiguous_prelude_variant(&call.callee, &variants);
            }
            return Err(());
        };
        let mut probes = probes;
        let probe = probes.swap_remove(winner);
        let commit = commits.swap_remove(winner);
        let expression = match (probe, commit) {
            (NamedFunctionLikeProbe::Callable(probe), NamedFunctionCommit::TopLevel) => {
                let resolved = self.commit_named_callable(*probe, sink).ok_or(())?;
                self.finish_resolved_top_level_function_call(call, resolved, sink)
            }
            (NamedFunctionLikeProbe::Callable(probe), NamedFunctionCommit::Member) => {
                let resolved = self.commit_named_callable(*probe, sink).ok_or(())?;
                self.finish_resolved_method_call(resolved, call.span)
            }
            (
                NamedFunctionLikeProbe::ImportedDependency(probe),
                NamedFunctionCommit::ImportedDependency,
            ) => return Ok(self.commit_imported_dependency_callable(*probe, sink)),
            (NamedFunctionLikeProbe::Nominal(probe), NamedFunctionCommit::Nominal) => {
                let resolved = self
                    .commit_named_nominal(*probe, call.span, sink)
                    .ok_or(())?;
                Some(self.finish_named_nominal(resolved, call.span))
            }
            (NamedFunctionLikeProbe::IntrinsicStruct(_), NamedFunctionCommit::Intrinsic(layer)) => {
                Some(self.commit_expr_layer(layer, sink))
            }
            _ => unreachable!("each typed probe retains its matching commit protocol"),
        };
        expression.map(Some).ok_or(())
    }
}
