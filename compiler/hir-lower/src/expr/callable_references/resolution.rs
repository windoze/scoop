//! Candidate layering and expected-type-driven callable-reference resolution.

use super::*;

use crate::call_resolution::applicability::CallableReferenceApplicabilityInput;

struct ApplicableReference {
    candidate: crate::CallableCandidate,
    type_args: Vec<TypeId>,
    ty: TypeId,
    own_type_param_count: usize,
}

impl Lowerer {
    pub(super) fn is_declared_type_name(&self, name: &str) -> bool {
        self.classes_by_name.contains_key(name)
            || self.interfaces_by_name.contains_key(name)
            || self.structs_by_name.contains_key(name)
            || self.enums_by_name.contains_key(name)
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
        expected: Option<&(TypeId, hir::FunctionType)>,
        display: &str,
        span: Span,
        extension_mode: ReferenceExtensionMode,
    ) -> Option<ResolvedReference> {
        let candidates = candidates
            .iter()
            .copied()
            .map(|function| crate::CallableCandidate::function(function, owner_type_args.to_vec()))
            .collect::<Vec<_>>();
        self.resolve_reference_candidate_set(&candidates, expected, display, span, extension_mode)
    }

    pub(super) fn resolve_member_reference_candidates(
        &mut self,
        candidates: &[crate::CallableCandidate],
        expected: Option<&(TypeId, hir::FunctionType)>,
        display: &str,
        span: Span,
    ) -> Option<ResolvedReference> {
        self.resolve_reference_candidate_set(
            candidates,
            expected,
            display,
            span,
            ReferenceExtensionMode::Exclude,
        )
    }

    fn resolve_reference_candidate_set(
        &mut self,
        candidates: &[crate::CallableCandidate],
        expected: Option<&(TypeId, hir::FunctionType)>,
        display: &str,
        span: Span,
        extension_mode: ReferenceExtensionMode,
    ) -> Option<ResolvedReference> {
        let mut applicable = Vec::new();
        for candidate in candidates {
            let function = candidate.function;
            let owner_type_args = self.callable_candidate_owner_arguments(candidate);
            let extension_receiver = self.extension_receivers.get(&function).copied();
            let bound_receiver = match extension_mode {
                ReferenceExtensionMode::Exclude if extension_receiver.is_some() => continue,
                ReferenceExtensionMode::Bound(_) if extension_receiver.is_none() => continue,
                ReferenceExtensionMode::Bound(receiver) => Some(receiver),
                ReferenceExtensionMode::Exclude | ReferenceExtensionMode::IncludeUnbound => None,
            };
            let view = self.callable_view(candidate, extension_receiver.is_some());
            if view.owner_parameters.len() != owner_type_args.len() {
                continue;
            }
            let own_type_param_count = view.callable_parameters.len();
            if expected.is_none() && own_type_param_count != 0 {
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
                        .map(|&parameter| self.instantiate_ty(parameter, &owner_type_args))
                        .collect();
                    let return_type = self.instantiate_ty(view.return_type, &owner_type_args);
                    self.intern_function_type(view.effects.is_suspend, parameters, return_type)
                },
                |(ty, _)| *ty,
            );
            let Ok(type_args) =
                self.solve_callable_reference_applicability(CallableReferenceApplicabilityInput {
                    view: &view,
                    owner_arguments: &owner_type_args,
                    bound_receiver,
                    parameter_types: &reference_params,
                    expected_type,
                })
            else {
                continue;
            };
            applicable.push(ApplicableReference {
                candidate: candidate.clone(),
                type_args,
                ty: expected_type,
                own_type_param_count,
            });
        }

        let selected = match applicable.len() {
            0 => {
                let message = if expected.is_some() {
                    format!("no overload of {display} matches the expected function type")
                } else {
                    format!(
                        "cannot determine {display} without an expected function type; no unique non-generic candidate exists"
                    )
                };
                self.error(span, message);
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
                    self.error(
                        span,
                        format!("{display} is ambiguous for the expected function type"),
                    );
                    return None;
                }
            }
            _ => {
                self.error(
                    span,
                    format!("{display} is ambiguous; provide an expected function type"),
                );
                return None;
            }
        };
        let selected = applicable.swap_remove(selected);
        let source = selected.candidate.source;
        let callee = self.materialize_candidate_callable(&selected.candidate, &selected.type_args);
        if !self.managed_reference_target_is_safe(callee, span) {
            return None;
        }
        Some(ResolvedReference {
            callable: callee,
            source,
            type_args: selected.type_args,
            ty: selected.ty,
        })
    }

    pub(in crate::expr) fn top_level_candidate_layer(&self, name: &str) -> Vec<hir::FunctionId> {
        self.candidate_layer(self.functions_by_name.get(name))
    }

    pub(super) fn managed_reference_target_is_safe(
        &mut self,
        callee: hir::Callable,
        span: Span,
    ) -> bool {
        let function = self.callable_function_id(callee);
        if self.functions[function].attributes.safety == hir::Safety::Safe {
            return true;
        }
        self.error(
            span,
            format!(
                "unsafe function `{}` cannot be stored in a managed function type because safety is not part of function-type identity",
                self.functions[function].name
            ),
        );
        false
    }

    pub(in crate::expr) fn extension_candidate_layer(&self, name: &str) -> Vec<hir::FunctionId> {
        self.candidate_layer(self.extensions_by_name.get(name))
    }

    fn candidate_layer(&self, ids: Option<&Vec<hir::FunctionId>>) -> Vec<hir::FunctionId> {
        let Some(ids) = ids else {
            return Vec::new();
        };
        let call_site_is_core = self.current_file < self.user_file_index;
        let same_side: Vec<_> = ids
            .iter()
            .copied()
            .filter(|id| (self.function_files[id] < self.user_file_index) == call_site_is_core)
            .collect();
        if same_side.is_empty() {
            ids.iter()
                .copied()
                .filter(|id| (self.function_files[id] < self.user_file_index) != call_site_is_core)
                .collect()
        } else {
            same_side
        }
    }

    pub(super) fn named_reference_candidate_layer(&self, name: &str) -> Vec<hir::FunctionId> {
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
        let call_site_is_core = self.current_file < self.user_file_index;
        let same_side: Vec<_> = ids
            .iter()
            .copied()
            .filter(|id| (self.function_files[id] < self.user_file_index) == call_site_is_core)
            .collect();
        let mut selected = if same_side.is_empty() {
            ids.into_iter()
                .filter(|id| (self.function_files[id] < self.user_file_index) != call_site_is_core)
                .collect::<Vec<_>>()
        } else {
            same_side
        };
        selected.sort_by_key(|id| id.into_raw().into_u32());
        selected
    }
}
