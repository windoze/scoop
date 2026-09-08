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
            || self.classes_by_name.contains_key(name)
            || self.interfaces_by_name.contains_key(name)
            || self.structs_by_name.contains_key(name)
            || self.enums_by_name.contains_key(name)
            || (self.source_type_alias_named(name).is_some() && self.type_alias_is_accessible(name))
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
    ) -> Option<ResolvedReference> {
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
    ) -> Option<ResolvedReference> {
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
    ) -> Option<ResolvedReference> {
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
            let owner_type_args = state.callable_candidate_owner_arguments(candidate);
            let extension_receiver = state.extension_receivers.get(&function).copied();
            let bound_receiver = match extension_mode {
                ReferenceExtensionMode::Exclude if extension_receiver.is_some() => continue,
                ReferenceExtensionMode::Bound(_) if extension_receiver.is_none() => continue,
                ReferenceExtensionMode::Bound(receiver) => Some(receiver),
                ReferenceExtensionMode::Exclude | ReferenceExtensionMode::IncludeUnbound => None,
            };
            let view = state.callable_view(candidate, extension_receiver.is_some());
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
                type_args,
                ty: expected_type,
                own_type_param_count,
            });
        }

        let selected = match applicable.len() {
            0 => {
                self.reference_failures_diagnostic(name, display, &failures, span);
                return None;
            }
            1 => 0,
            _ if expected.is_some() => {
                let concrete: Vec<_> = applicable
                    .iter()
                    .enumerate()
                    .filter_map(|(index, candidate)| {
                        (candidate.own_type_param_count == 0).then_some(index)
                    })
                    .collect();
                if let [index] = concrete.as_slice() {
                    *index
                } else {
                    self.reference_ambiguity_diagnostic(
                        name,
                        display,
                        &applicable,
                        expected.is_some(),
                        span,
                    );
                    return None;
                }
            }
            _ => {
                self.reference_ambiguity_diagnostic(
                    name,
                    display,
                    &applicable,
                    expected.is_some(),
                    span,
                );
                return None;
            }
        };
        let selected = applicable.swap_remove(selected);
        *self = *selected.state;
        let source = selected.candidate.source;
        let callee = self.materialize_candidate_callable(&selected.candidate, &selected.type_args);
        Some(ResolvedReference {
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

    pub(crate) fn extension_candidate_layers(&self, name: &str) -> Vec<Vec<hir::FunctionId>> {
        self.candidate_layers(self.extensions_by_name.get(name))
    }

    pub(in crate::expr) fn extension_operator_candidate_layers(
        &self,
        operator: hir::OperatorKind,
    ) -> Vec<Vec<hir::FunctionId>> {
        let mut ids = self
            .extensions_by_name
            .values()
            .flatten()
            .copied()
            .filter(|function| self.signatures[function].modifiers.operator == Some(operator))
            .collect::<Vec<_>>();
        ids.sort_by_key(|id| id.into_raw().into_u32());
        self.candidate_layers(Some(&ids))
    }

    fn candidate_layers(&self, ids: Option<&Vec<hir::FunctionId>>) -> Vec<Vec<hir::FunctionId>> {
        let Some(ids) = ids else {
            return Vec::new();
        };
        let same_side: Vec<_> = ids
            .iter()
            .copied()
            .filter(|id| self.function_is_accessible(*id, None))
            .filter(|id| self.sources_are_on_same_side(self.current_file, self.function_files[id]))
            .collect();
        let imported = ids
            .iter()
            .copied()
            .filter(|id| self.function_is_accessible(*id, None))
            .filter(|id| !self.sources_are_on_same_side(self.current_file, self.function_files[id]))
            .collect::<Vec<_>>();
        [same_side, imported]
            .into_iter()
            .filter(|layer| !layer.is_empty())
            .collect()
    }

    pub(super) fn named_reference_candidate_layers(&self, name: &str) -> Vec<Vec<hir::FunctionId>> {
        let mut ids = Vec::new();
        ids.extend(
            self.functions_by_name
                .get(name)
                .into_iter()
                .flatten()
                .copied(),
        );
        ids.extend(
            self.extensions_by_name
                .get(name)
                .into_iter()
                .flatten()
                .copied(),
        );
        let same_side: Vec<_> = ids
            .iter()
            .copied()
            .filter(|id| self.function_is_accessible(*id, None))
            .filter(|id| self.sources_are_on_same_side(self.current_file, self.function_files[id]))
            .collect();
        let imported = ids
            .into_iter()
            .filter(|id| self.function_is_accessible(*id, None))
            .filter(|id| !self.sources_are_on_same_side(self.current_file, self.function_files[id]))
            .collect::<Vec<_>>();
        [same_side, imported]
            .into_iter()
            .filter(|layer| !layer.is_empty())
            .map(|mut layer| {
                layer.sort_by_key(|id| id.into_raw().into_u32());
                layer
            })
            .collect()
    }
}
