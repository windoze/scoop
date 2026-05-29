//! Resolves dispatch targets: finds the concrete callable body for a given direct-call FQN, receiver type, or bound non-generic call.

use super::*;

impl MirInstanceMaterializer {
    pub(super) fn fun_decl_is_generic_family(&self, source_path: &Path, fun: &FunDecl) -> bool {
        if type_contains_param(&self.types, fun.ty)
            || function_type_has_effect_param(&self.types, fun.ty)
        {
            return true;
        }

        self.template_signatures.contains_key(&TemplateKey {
            fqn: fun.fqn.clone(),
            source_path: source_path.to_path_buf(),
            decl_span: fun.span,
        })
    }

    pub(super) fn reachable_fun_is_generic_family(&self, fun: &ReachableMirFun) -> bool {
        self.fun_decl_is_generic_family(fun.source_path.as_path(), &fun.fun)
    }

    pub(super) fn resolve_non_generic_fun_body_by_fqn(
        &self,
        default_source_path: &Path,
        fqn: &str,
    ) -> Option<ReachableMirFun> {
        if let Some(candidates) = self.reachable_fun_bodies_by_fqn.get(fqn) {
            if candidates.len() != 1 {
                return None;
            }
            let candidate = candidates[0].clone();
            return (!self.reachable_fun_is_generic_family(&candidate)).then_some(candidate);
        }
        let candidates = self.all_fun_bodies_by_fqn.get(fqn)?;
        if candidates.len() != 1 {
            return None;
        }
        let fun = candidates[0].clone();
        let reachable = ReachableMirFun {
            source_path: default_source_path.to_path_buf(),
            fun,
        };
        (!self.reachable_fun_is_generic_family(&reachable)).then_some(reachable)
    }

    pub(super) fn pass_visible_non_generic_callable_fqn(
        &self,
        source_path: &Path,
        fun: &FunDecl,
    ) -> String {
        let overloaded = self.generic_family_fqns.contains(&fun.fqn)
            || self
                .all_fun_bodies_by_fqn
                .get(&fun.fqn)
                .map(|candidates| {
                    candidates
                        .iter()
                        .filter(|candidate| {
                            !self.fun_decl_is_generic_family(source_path, candidate)
                        })
                        .count()
                        > 1
                })
                .unwrap_or(false);
        if !overloaded {
            return fun.fqn.clone();
        }
        let signature_key =
            canonical_callable_signature_key(&self.types, fun.ty, 0, 0, 0, &NoTypeParamResolver)
                .unwrap_or_else(|err| {
                    panic!(
                        "failed to build non-generic overload signature key for `{}`: {err}",
                        fun.fqn
                    )
                });
        let stable_template_key = stable_template_key_for_template(
            &self.stable_cone_key,
            &fun.fqn,
            StableDefNamespace::Fun,
            "non_generic_overload",
            &signature_key,
        );
        format!(
            "{}$overload${}",
            fun.fqn,
            stable_template_symbol_suffix(&stable_template_key)
        )
    }

    pub(super) fn non_generic_direct_callee_receiver_matches(
        &self,
        fun: &FunDecl,
        receiver_ty: TypeId,
    ) -> bool {
        if fun
            .params
            .first()
            .is_some_and(|param| param.ty == receiver_ty)
        {
            return true;
        }
        if fun.params.first().is_some_and(|param| {
            nominal_type_fqn(&self.types, param.ty) == nominal_type_fqn(&self.types, receiver_ty)
        }) {
            return true;
        }
        let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(fun.ty) else {
            return false;
        };
        let Some(declared_receiver) = fun_ty.receiver else {
            return false;
        };
        nominal_type_fqn(&self.types, declared_receiver)
            == nominal_type_fqn(&self.types, receiver_ty)
    }

    pub(super) fn resolve_non_generic_fun_body_by_receiver(
        &self,
        default_source_path: &Path,
        fqn: &str,
        receiver_ty: TypeId,
    ) -> Option<ReachableMirFun> {
        if let Some(candidates) = self.reachable_fun_bodies_by_fqn.get(fqn) {
            let matching = candidates
                .iter()
                .filter(|candidate| {
                    !self.reachable_fun_is_generic_family(candidate)
                        && self
                            .non_generic_direct_callee_receiver_matches(&candidate.fun, receiver_ty)
                })
                .cloned()
                .collect::<Vec<_>>();
            return (matching.len() == 1).then(|| matching.into_iter().next().unwrap());
        }
        let candidates = self.all_fun_bodies_by_fqn.get(fqn)?;
        let matching = candidates
            .iter()
            .filter(|candidate| {
                !self.fun_decl_is_generic_family(default_source_path, candidate)
                    && self.non_generic_direct_callee_receiver_matches(candidate, receiver_ty)
            })
            .cloned()
            .collect::<Vec<_>>();
        (matching.len() == 1).then(|| ReachableMirFun {
            source_path: default_source_path.to_path_buf(),
            fun: matching.into_iter().next().unwrap(),
        })
    }

    pub(super) fn resolve_bound_non_generic_fun_call(
        &self,
        template_source_path: &Path,
        enclosing_span: Span,
        callee_fqn: &str,
    ) -> Option<ReachableMirFun> {
        let binding = lookup_overlapping_direct_call_binding(
            &self.direct_call_bindings,
            template_source_path,
            enclosing_span,
        )?;
        if binding.is_intrinsic
            || !binding.type_args.is_empty()
            || !binding.eff_args.is_empty()
            || binding.fqn != callee_fqn
        {
            return None;
        }
        self.reachable_fun_bodies_by_request
            .get(&(
                binding.fqn.clone(),
                binding.decl_file.clone(),
                binding.decl_span,
            ))
            .cloned()
            .filter(|fun| !self.reachable_fun_is_generic_family(fun))
    }

    pub(super) fn resolve_non_generic_direct_callee(
        &self,
        template_source_path: &Path,
        call_span: Span,
        callee_fqn: &str,
        args: &[CallArg],
        locals: &[LocalDecl],
    ) -> Option<ReachableMirFun> {
        if let Some(fun) =
            self.resolve_bound_non_generic_fun_call(template_source_path, call_span, callee_fqn)
        {
            return Some(fun);
        }

        if let Some(fun) =
            self.resolve_non_generic_fun_body_by_fqn(template_source_path, callee_fqn)
        {
            return Some(fun);
        }

        let receiver_ty = args
            .first()
            .and_then(|arg| operand_type(&self.types, self.builtins, locals, &arg.value))?;
        self.resolve_non_generic_fun_body_by_receiver(template_source_path, callee_fqn, receiver_ty)
    }

    pub(super) fn resolve_non_generic_top_level_ref_target(
        &self,
        template_source_path: &Path,
        enclosing_span: Span,
        fqn: &str,
    ) -> Option<ReachableMirFun> {
        self.resolve_bound_non_generic_fun_call(template_source_path, enclosing_span, fqn)
            .or_else(|| self.resolve_non_generic_fun_body_by_fqn(template_source_path, fqn))
    }
}
