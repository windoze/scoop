//! Candidate layering and expected-type-driven callable-reference resolution.

use super::*;

use crate::call_resolution::applicability::CallableReferenceApplicabilityInput;
use crate::call_resolution::candidates::CallableView;
use crate::call_resolution::constraints::ConstraintFailure;
use crate::call_resolution::diagnostics::{
    callable_layer_name, callable_source_signature, render_callable_constraint_failure,
};

struct ApplicableReference {
    state: Box<Lowerer>,
    candidate: crate::CallableCandidate,
    view: CallableView,
    forwarding_parameter_types: Vec<TypeId>,
    type_args: Vec<TypeId>,
    ty: TypeId,
    own_type_param_count: usize,
}

struct ReferenceFailure {
    state: Box<Lowerer>,
    view: CallableView,
    kind: ReferenceFailureKind,
}

enum ReferenceFailureKind {
    MissingExpectedType,
    IncompleteOwnerArguments,
    UnsafeManagedTarget,
    Constraint(ConstraintFailure),
}

impl Lowerer {
    pub(super) fn is_declared_type_name(&self, name: &str) -> bool {
        self.lexical_nested_nominal_target(name).is_some()
            || self.top_level_type_target(name).is_some()
    }

    pub(super) fn expected_function_signature(
        &self,
        expected: Option<TypeId>,
    ) -> Option<(TypeId, hir::FunctionType)> {
        expected.and_then(|ty| match self.types[ty] {
            Type::Function(id) => Some((ty, self.function_types[id].clone())),
            _ => None,
        })
    }

    pub(super) fn resolve_reference_candidates(
        &mut self,
        candidates: &[hir::FunctionId],
        owner_type_args: &[TypeId],
        context: ReferenceResolutionContext<'_>,
    ) -> ReferenceResolutionOutcome {
        let candidates = candidates
            .iter()
            .copied()
            .map(|function| {
                crate::CallableCandidate::function(
                    function,
                    owner_type_args.to_vec(),
                    self.function_lookup_witness(function),
                )
            })
            .collect::<Vec<_>>();
        self.resolve_reference_candidate_set(&candidates, context)
    }

    pub(super) fn resolve_member_reference_candidates(
        &mut self,
        candidates: &[crate::CallableCandidate],
        context: ReferenceResolutionContext<'_>,
    ) -> ReferenceResolutionOutcome {
        debug_assert!(matches!(
            context.extension_mode,
            ReferenceExtensionMode::Exclude
        ));
        self.resolve_reference_candidate_set(candidates, context)
    }

    fn resolve_reference_candidate_set(
        &mut self,
        candidates: &[crate::CallableCandidate],
        context: ReferenceResolutionContext<'_>,
    ) -> ReferenceResolutionOutcome {
        let suppressed = candidates.iter().any(|candidate| {
            self.declaration_surface
                .rejects_function(candidate.function)
        });
        let candidates = candidates
            .iter()
            .filter(|candidate| {
                !self
                    .declaration_surface
                    .rejects_function(candidate.function)
            })
            .cloned()
            .collect::<Vec<_>>();
        if candidates.is_empty() && suppressed {
            return ReferenceResolutionOutcome::Blocked;
        }
        let ReferenceResolutionContext {
            expected,
            name,
            display,
            span,
            extension_mode,
        } = context;
        let mut applicable = Vec::new();
        let mut failures = Vec::new();
        for candidate in candidates {
            let mut state = self.clone();
            let function = candidate.function;
            let owner_type_args = state.callable_candidate_owner_arguments(&candidate);
            let extension_receiver = state.extension_receivers.get(&function).copied();
            let bound_receiver = match extension_mode {
                ReferenceExtensionMode::Exclude if extension_receiver.is_some() => continue,
                ReferenceExtensionMode::Bound(_) if extension_receiver.is_none() => continue,
                ReferenceExtensionMode::Bound(receiver) => Some(receiver),
                ReferenceExtensionMode::Exclude | ReferenceExtensionMode::IncludeUnbound => None,
            };
            let view = state.callable_view(&candidate, extension_receiver.is_some());
            if view.owner_parameters.len() != owner_type_args.len() {
                failures.push(ReferenceFailure {
                    state: Box::new(state),
                    view,
                    kind: ReferenceFailureKind::IncompleteOwnerArguments,
                });
                continue;
            }
            let own_type_param_count = view.callable_parameters.len();
            if expected.is_none() && own_type_param_count != 0 {
                failures.push(ReferenceFailure {
                    state: Box::new(state),
                    view,
                    kind: ReferenceFailureKind::MissingExpectedType,
                });
                continue;
            }
            if state.functions[function].attributes.safety == hir::Safety::Unsafe {
                failures.push(ReferenceFailure {
                    state: Box::new(state),
                    view,
                    kind: ReferenceFailureKind::UnsafeManagedTarget,
                });
                continue;
            }

            let mut reference_params: Vec<_> = view
                .value_parameters
                .iter()
                .map(|parameter| parameter.ty)
                .collect();
            let mut forwarding_parameter_types = reference_params.clone();
            if let Some(receiver) = extension_receiver {
                forwarding_parameter_types.insert(0, receiver);
            }
            if matches!(extension_mode, ReferenceExtensionMode::IncludeUnbound)
                && let Some(receiver) = extension_receiver
            {
                reference_params.insert(0, receiver);
            }

            let expected_type = expected.map_or_else(
                || {
                    let parameters = reference_params
                        .iter()
                        .map(|&parameter| state.instantiate_ty(parameter, &owner_type_args))
                        .collect();
                    let return_type = state.instantiate_ty(view.return_type, &owner_type_args);
                    state.intern_function_type(view.effects.is_suspend, parameters, return_type)
                },
                |(ty, _)| *ty,
            );
            let type_args = match state.solve_callable_reference_applicability(
                CallableReferenceApplicabilityInput {
                    view: &view,
                    owner_arguments: &owner_type_args,
                    bound_receiver,
                    parameter_types: &reference_params,
                    expected_type,
                },
            ) {
                Ok(type_args) => type_args,
                Err(failure) => {
                    failures.push(ReferenceFailure {
                        state: Box::new(state),
                        view,
                        kind: ReferenceFailureKind::Constraint(failure),
                    });
                    continue;
                }
            };
            applicable.push(ApplicableReference {
                state: Box::new(state),
                candidate: candidate.clone(),
                view,
                forwarding_parameter_types,
                type_args,
                ty: expected_type,
                own_type_param_count,
            });
        }

        let selected = match applicable.len() {
            0 => {
                self.reference_failures_diagnostic(name, display, &failures, span);
                return if suppressed {
                    ReferenceResolutionOutcome::Failed
                } else {
                    ReferenceResolutionOutcome::NoApplicable
                };
            }
            1 => 0,
            _ if expected.is_none() => {
                self.reference_ambiguity_diagnostic(name, display, &applicable, false, span);
                return ReferenceResolutionOutcome::Failed;
            }
            _ => {
                let mut pool = (0..applicable.len())
                    .filter(|&candidate| {
                        !(0..applicable.len()).any(|other| {
                            if other == candidate {
                                return false;
                            }
                            let preferred =
                                crate::call_resolution::specificity::ForwardingDeclaration {
                                    view: &applicable[other].view,
                                    parameter_types: &applicable[other].forwarding_parameter_types,
                                };
                            let displaced =
                                crate::call_resolution::specificity::ForwardingDeclaration {
                                    view: &applicable[candidate].view,
                                    parameter_types: &applicable[candidate]
                                        .forwarding_parameter_types,
                                };
                            self.callable_forwards(preferred, displaced)
                                && !self.callable_forwards(displaced, preferred)
                        })
                    })
                    .collect::<Vec<_>>();
                if pool
                    .iter()
                    .any(|&candidate| applicable[candidate].own_type_param_count == 0)
                {
                    pool.retain(|&candidate| applicable[candidate].own_type_param_count == 0);
                }
                if let [winner] = pool.as_slice() {
                    *winner
                } else {
                    let ambiguous = pool
                        .into_iter()
                        .map(|index| ApplicableReference {
                            state: applicable[index].state.clone(),
                            candidate: applicable[index].candidate.clone(),
                            view: applicable[index].view.clone(),
                            forwarding_parameter_types: applicable[index]
                                .forwarding_parameter_types
                                .clone(),
                            type_args: applicable[index].type_args.clone(),
                            ty: applicable[index].ty,
                            own_type_param_count: applicable[index].own_type_param_count,
                        })
                        .collect::<Vec<_>>();
                    self.reference_ambiguity_diagnostic(
                        name,
                        display,
                        &ambiguous,
                        expected.is_some(),
                        span,
                    );
                    return ReferenceResolutionOutcome::Failed;
                }
            }
        };
        let selected = applicable.swap_remove(selected);
        *self = *selected.state;
        let source = selected.candidate.source;
        let callee = self.materialize_candidate_callable(&selected.candidate, &selected.type_args);
        ReferenceResolutionOutcome::Resolved(ResolvedReference {
            callable: callee,
            source,
            type_args: selected.type_args,
            ty: selected.ty,
        })
    }

    fn reference_failures_diagnostic(
        &mut self,
        name: &str,
        display: &str,
        failures: &[ReferenceFailure],
        span: Span,
    ) {
        if failures.is_empty() {
            self.error(
                span,
                format!("no declaration candidate has a permitted form for {display}"),
            );
            return;
        }
        let views = failures
            .iter()
            .map(|failure| failure.view.clone())
            .collect::<Vec<_>>();
        let layer = callable_layer_name(self, &views);
        let traces = failures
            .iter()
            .map(|failure| {
                let signature = callable_source_signature(self, name, &failure.view);
                let reason = match &failure.kind {
                    ReferenceFailureKind::MissingExpectedType => {
                        "generic callable references require an expected function type".to_string()
                    }
                    ReferenceFailureKind::IncompleteOwnerArguments => {
                        "receiver does not provide complete owner type arguments".to_string()
                    }
                    ReferenceFailureKind::UnsafeManagedTarget => {
                        "unsafe functions cannot be stored in a managed function type because safety is not part of function-type identity".to_string()
                    }
                    ReferenceFailureKind::Constraint(constraint) => {
                        render_callable_constraint_failure(
                            &failure.state,
                            &failure.view,
                            None,
                            &[],
                            constraint,
                        )
                    }
                };
                format!("  - {signature} — {reason}")
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.error(
            span,
            format!("no applicable candidate for {display} in {layer} layer:\n{traces}"),
        );
    }

    fn reference_ambiguity_diagnostic(
        &mut self,
        name: &str,
        display: &str,
        applicable: &[ApplicableReference],
        has_expected_type: bool,
        span: Span,
    ) {
        let views = applicable
            .iter()
            .map(|candidate| candidate.view.clone())
            .collect::<Vec<_>>();
        let layer = callable_layer_name(self, &views);
        let reason = if has_expected_type {
            "matches the expected function type"
        } else {
            "forms a complete non-generic function type"
        };
        let traces = applicable
            .iter()
            .map(|candidate| {
                format!(
                    "  - {} — {reason}",
                    callable_source_signature(self, name, &candidate.view)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.error(
            span,
            format!("{display} is ambiguous in {layer} layer:\n{traces}"),
        );
    }

    pub(super) fn named_reference_candidate_layers(
        &self,
        name: &str,
    ) -> Vec<crate::imports::lookup::LookupLayer<hir::FunctionId>> {
        self.named_callable_reference_layers(name)
            .into_iter()
            .map(|mut layer| {
                layer.candidates.sort_by_key(|id| id.into_raw().into_u32());
                layer
            })
            .filter(|layer| !layer.candidates.is_empty() || !layer.suppressed_callables.is_empty())
            .collect()
    }
}
