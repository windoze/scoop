//! LLVM codegen bridge into the shared effect state-machine skeleton.

use std::cell::Ref;
use std::collections::HashMap;
use std::rc::Rc;

use crate::effect::analysis::{ContinuationEscapeFacts, EffectAnalysisCtx, KnownLocalMetadata};
use crate::effect::state_machine::{
    self, CalleeSuspendPlan, SuspendCallAnalysis, collect_known_fun_call_suspendability,
    function_ty_declared_effectful, hir_ty_is_function_value,
};
use crate::ty::TypeId;

use super::*;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn effect_analysis_ctx(&self) -> EffectAnalysisCtx {
        let known_fun_effects = self.known_fun_call_suspendability_map().clone();
        let mut known_local_fun_effects = HashMap::new();
        let mut known_local_metadata = HashMap::new();
        for scope in &self.function_cx.env.scopes {
            for (&id, local) in scope {
                let Some(hir_ty) = local.hir_ty else {
                    continue;
                };
                known_local_metadata.insert(
                    id,
                    KnownLocalMetadata {
                        ty: hir_ty,
                        mutable: local.mutable,
                    },
                );
                if hir_ty_is_function_value(self.types, hir_ty) {
                    known_local_fun_effects.insert(id, local.call_may_suspend);
                }
            }
        }
        let current_source_path = self
            .current_source()
            .expect("codegen context should always have a current source")
            .path()
            .to_path_buf();
        EffectAnalysisCtx::new(
            known_fun_effects,
            known_local_fun_effects,
            known_local_metadata,
            current_source_path,
            Rc::clone(&self.shared.program_facts),
        )
        .with_continuation_escape_facts(
            ContinuationEscapeFacts::from_pass_view_for_callable(
                self.materialized_pass_view(),
                self.function_cx.current_callable_fqn.as_deref(),
                self.current_source()
                    .expect("codegen context should always have a current source")
                    .path(),
            ),
        )
    }

    pub(in crate::llvm::codegen) fn build_ordinary_callee_suspend_plan_from_unified_contract(
        &self,
        body: &hir::Block,
        declared_return_ty: TypeId,
    ) -> Option<CalleeSuspendPlan> {
        let mut context = self.effect_analysis_ctx();
        state_machine::build_ordinary_callee_suspend_plan_with_context(
            self.types,
            body,
            declared_return_ty,
            &mut context,
        )
    }

    fn ensure_known_fun_body_may_outward_effect_cache(&self) {
        if self
            .shared_caches
            .known_fun_call_suspend_cache
            .borrow()
            .is_some()
        {
            return;
        }

        let known_fun_effects = collect_known_fun_call_suspendability(
            self.types,
            self.fun_index,
            Rc::clone(&self.shared.program_facts),
            self.materialized_pass_view(),
        );
        *self.shared_caches.known_fun_call_suspend_cache.borrow_mut() = Some(known_fun_effects);
    }

    fn known_fun_body_may_outward_effect_map(&self) -> Ref<'_, HashMap<String, bool>> {
        self.ensure_known_fun_body_may_outward_effect_cache();
        Ref::map(
            self.shared_caches.known_fun_call_suspend_cache.borrow(),
            |cache| {
                cache
                    .as_ref()
                    .expect("known fun outward-effect cache should be initialized")
            },
        )
    }

    fn known_fun_call_suspendability_map(&self) -> Ref<'_, HashMap<String, bool>> {
        self.known_fun_body_may_outward_effect_map()
    }

    pub(in crate::llvm::codegen) fn known_fun_body_may_outward_effect(
        &self,
        fqn: &str,
        declared_fun_ty: TypeId,
    ) -> bool {
        let known_fun_effects = self.known_fun_body_may_outward_effect_map();
        known_fun_effects
            .get(fqn)
            .copied()
            .unwrap_or_else(|| function_ty_declared_effectful(self.types, declared_fun_ty))
    }

    pub(in crate::llvm::codegen) fn hir_ty_declared_effectful(
        &self,
        hir_ty: Option<TypeId>,
    ) -> bool {
        hir_ty.is_some_and(|ty| function_ty_declared_effectful(self.types, ty))
    }

    pub(in crate::llvm::codegen) fn local_call_may_suspend_from_hir_ty(
        &self,
        hir_ty: Option<TypeId>,
    ) -> bool {
        self.hir_ty_declared_effectful(hir_ty)
    }

    pub(in crate::llvm::codegen) fn function_value_expr_body_may_outward_effect_when_called_for_local(
        &self,
        expr: &hir::Expr,
    ) -> bool {
        let context = self.effect_analysis_ctx();
        SuspendCallAnalysis {
            types: self.types,
            context: &context,
        }
        .function_value_may_suspend_when_called(expr, &context.known_local_fun_effects)
    }

    pub(super) fn build_unified_lowering_contract(
        &self,
        handle: &hir::HandleExpr,
    ) -> state_machine::UnifiedHandleLoweringContract {
        let mut context = self.effect_analysis_ctx();
        state_machine::build_unified_lowering_contract(self.types, handle, &mut context)
    }
}
