//! Candidate layering and expected-type-driven callable-reference resolution.

use super::*;

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
            let sig = self.signatures[&function].clone();
            if sig.owner_type_param_count != owner_type_args.len() {
                continue;
            }
            let mut bindings = vec![None; sig.type_params.len()];
            for (binding, &ty) in bindings.iter_mut().zip(&owner_type_args) {
                *binding = Some(ty);
            }
            let extension_receiver = self.extension_receivers.get(&function).copied();
            let bound_extension_receiver = match extension_mode {
                ReferenceExtensionMode::Exclude if extension_receiver.is_some() => continue,
                ReferenceExtensionMode::Bound(_) if extension_receiver.is_none() => continue,
                ReferenceExtensionMode::Bound(receiver) => Some(receiver),
                ReferenceExtensionMode::Exclude | ReferenceExtensionMode::IncludeUnbound => None,
            };
            if let (Some(declared), Some(actual)) = (extension_receiver, bound_extension_receiver)
                && !self.try_bind(declared, actual, &mut bindings)
            {
                continue;
            }
            let mut reference_params: Vec<_> =
                sig.params.iter().map(|parameter| parameter.ty).collect();
            if matches!(extension_mode, ReferenceExtensionMode::IncludeUnbound)
                && let Some(receiver) = extension_receiver
            {
                reference_params.insert(0, receiver);
            }
            match expected {
                Some((_, expected)) => {
                    if sig.is_suspend != expected.is_suspend
                        || reference_params.len() != expected.parameter_types.len()
                    {
                        continue;
                    }
                    let parameters_match = reference_params
                        .iter()
                        .zip(&expected.parameter_types)
                        .all(|(&parameter, &expected)| {
                            self.try_bind(parameter, expected, &mut bindings)
                        });
                    if !parameters_match
                        || !self.try_bind(sig.return_ty, expected.return_type, &mut bindings)
                        || bindings.iter().any(Option::is_none)
                    {
                        continue;
                    }
                }
                None if sig.type_params.len() != sig.owner_type_param_count => continue,
                None => {}
            }
            let type_args: Vec<_> = bindings.into_iter().flatten().collect();
            if !self.type_arguments_satisfy_kinds(&sig.type_params, &type_args) {
                continue;
            }
            let parameter_types: Vec<_> = reference_params
                .iter()
                .map(|&parameter| self.instantiate_ty(parameter, &type_args))
                .collect();
            let return_type = self.instantiate_ty(sig.return_ty, &type_args);
            if let (Some(declared), Some(actual)) = (extension_receiver, bound_extension_receiver) {
                let declared = self.instantiate_ty(declared, &type_args);
                if !self.is_subtype(actual, declared) {
                    continue;
                }
            }
            if let Some((_, expected)) = expected {
                let exact = parameter_types
                    .iter()
                    .zip(&expected.parameter_types)
                    .all(|(&parameter, &expected)| self.types_equal(parameter, expected))
                    && self.types_equal(return_type, expected.return_type);
                if !exact {
                    continue;
                }
            }
            let own_type_param_count = sig.type_params.len() - sig.owner_type_param_count;
            applicable.push((
                function,
                candidate.source,
                candidate.owner.clone(),
                type_args,
                parameter_types,
                return_type,
                sig.is_suspend,
                own_type_param_count,
            ));
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
                    .filter_map(|(index, candidate)| (candidate.7 == 0).then_some(index))
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
        let (function, source, owner, type_args, parameter_types, return_type, is_suspend, _) =
            applicable.swap_remove(selected);
        let ty = match expected {
            Some((ty, _)) => *ty,
            None => self.intern_function_type(is_suspend, parameter_types, return_type),
        };
        let selected_candidate = crate::CallableCandidate {
            function,
            owner,
            source,
        };
        let callee = self.materialize_candidate_callable(&selected_candidate, &type_args);
        if !self.managed_reference_target_is_safe(callee, span) {
            return None;
        }
        Some(ResolvedReference {
            callable: callee,
            source,
            type_args,
            ty,
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
